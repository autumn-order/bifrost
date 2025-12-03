//! Tests for EntityRefreshTracker
//!
//! These tests verify the entity refresh scheduling behavior including:
//! - Finding entries that need updating based on cache expiration
//! - Scheduling jobs for expired entries
//! - Handling empty tables
//! - Batch limiting
//! - Job scheduling with staggered execution times

use bifrost::server::{
    model::worker::WorkerJob,
    scheduler::{
        config::eve::alliance as alliance_config,
        entity_refresh::{EntityRefreshTracker, SchedulableEntity},
    },
};
use bifrost_test_utils::prelude::*;
use chrono::{Duration, Utc};
use entity::prelude::EveAlliance;
use migration::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::util::redis::RedisTest;
use crate::worker::queue::setup_test_queue;

pub struct AllianceInfo;

impl SchedulableEntity for AllianceInfo {
    type Entity = entity::eve_alliance::Entity;

    fn updated_at_column() -> impl ColumnTrait + sea_orm::IntoSimpleExpr {
        entity::eve_alliance::Column::UpdatedAt
    }

    fn id_column() -> impl ColumnTrait + sea_orm::IntoSimpleExpr {
        entity::eve_alliance::Column::AllianceId
    }
}

mod find_entries_needing_update {
    use super::*;

    /// Tests finding entries when database table is empty.
    ///
    /// Verifies that the entity refresh tracker correctly handles an empty table
    /// by returning an empty list without errors when no entries exist to update.
    ///
    /// Expected: Ok with empty Vec
    #[tokio::test]
    async fn returns_empty_when_no_entries() -> Result<(), TestError> {
        let test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let ids = result.unwrap();
        assert!(ids.is_empty());

        Ok(())
    }

    /// Tests finding entries when all have fresh cache.
    ///
    /// Verifies that the entity refresh tracker skips entries with recent updated_at
    /// timestamps and returns an empty list when all entries are within cache duration.
    ///
    /// Expected: Ok with empty Vec
    #[tokio::test]
    async fn returns_empty_when_all_up_to_date() -> Result<(), TestError> {
        let mut test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;
        test.eve().insert_mock_alliance(1, None).await?;

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let ids = result.unwrap();
        assert!(ids.is_empty());

        Ok(())
    }

    /// Tests finding entries with expired cache timestamps.
    ///
    /// Verifies that the entity refresh tracker correctly identifies entries whose
    /// updated_at timestamp exceeds the configured cache duration (24 hours for alliances)
    /// and returns their IDs for scheduling.
    ///
    /// Expected: Ok with Vec containing one alliance_id
    #[tokio::test]
    async fn returns_entries_with_expired_cache() -> Result<(), TestError> {
        let mut test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;
        let alliance = test.eve().insert_mock_alliance(1, None).await?;

        // Update the alliance to have an old updated_at timestamp
        let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
        EveAlliance::update_many()
            .col_expr(
                entity::eve_alliance::Column::UpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
            .exec(&test.db)
            .await?;

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], alliance.alliance_id);

        Ok(())
    }

    /// Tests that oldest entries are returned first.
    ///
    /// Verifies that the entity refresh tracker orders entries by updated_at timestamp
    /// in ascending order, prioritizing the oldest (most stale) entries first for
    /// optimal cache freshness management.
    ///
    /// Expected: Ok with Vec ordered by age (oldest to newest)
    #[tokio::test]
    async fn returns_oldest_updated_first() -> Result<(), TestError> {
        let mut test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;
        let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
        let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
        let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

        // Set different updated_at timestamps
        let oldest = Utc::now().naive_utc() - Duration::hours(72);
        let middle = Utc::now().naive_utc() - Duration::hours(48);
        let newest = Utc::now().naive_utc() - Duration::hours(25);

        EveAlliance::update_many()
            .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(middle))
            .filter(entity::eve_alliance::Column::Id.eq(alliance1.id))
            .exec(&test.db)
            .await?;

        EveAlliance::update_many()
            .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(oldest))
            .filter(entity::eve_alliance::Column::Id.eq(alliance2.id))
            .exec(&test.db)
            .await?;

        EveAlliance::update_many()
            .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(newest))
            .filter(entity::eve_alliance::Column::Id.eq(alliance3.id))
            .exec(&test.db)
            .await?;

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 3);
        // Should be ordered: alliance2 (oldest), alliance1 (middle), alliance3 (newest)
        assert_eq!(ids[0], alliance2.alliance_id);
        assert_eq!(ids[1], alliance1.alliance_id);
        assert_eq!(ids[2], alliance3.alliance_id);

        Ok(())
    }

    /// Tests batch limiting behavior.
    ///
    /// Verifies that the entity refresh tracker respects batch size calculations
    /// based on cache duration and schedule interval. With a 24h cache and 30min interval,
    /// the minimum batch limit (100) is applied when the calculated batch would be smaller.
    ///
    /// Expected: Ok with Vec containing all 10 entries (less than min limit of 100)
    #[tokio::test]
    async fn respects_batch_limit() -> Result<(), TestError> {
        let mut test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;

        // Create 10 alliances with expired cache
        let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
        for i in 1..=10 {
            let alliance = test.eve().insert_mock_alliance(i, None).await?;
            EveAlliance::update_many()
                .col_expr(
                    entity::eve_alliance::Column::UpdatedAt,
                    Expr::value(old_timestamp),
                )
                .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
                .exec(&test.db)
                .await?;
        }

        // With 10 entries, cache 24h, interval 30min = 10 / 48 = 0 -> min 100
        // But we only have 10 entries, so we should get all 10
        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let ids = result.unwrap();
        // Should return all 10 since that's less than MIN_BATCH_LIMIT (100)
        assert_eq!(ids.len(), 10);

        Ok(())
    }

    /// Tests finding a single expired entry.
    ///
    /// Verifies that the entity refresh tracker correctly handles the case where
    /// only one entry needs updating, returning a single-element list.
    ///
    /// Expected: Ok with Vec containing one alliance_id
    #[tokio::test]
    async fn handles_single_entry() -> Result<(), TestError> {
        let mut test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;
        let alliance = test.eve().insert_mock_alliance(1, None).await?;

        let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
        EveAlliance::update_many()
            .col_expr(
                entity::eve_alliance::Column::UpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
            .exec(&test.db)
            .await?;

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], alliance.alliance_id);

        Ok(())
    }

    /// Tests error handling when database tables are missing.
    ///
    /// Verifies that the entity refresh tracker returns an error when required
    /// database tables are not present in the database schema.
    ///
    /// Expected: Err
    #[tokio::test]
    async fn fails_when_tables_missing() -> Result<(), TestError> {
        let test = TestBuilder::new().build().await?;

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_err());

        Ok(())
    }
}

mod schedule_jobs {
    use super::*;

    /// Tests scheduling a single job.
    ///
    /// Verifies that the entity refresh tracker can schedule a single worker job
    /// to the queue and returns the count of scheduled jobs (1).
    ///
    /// Expected: Ok(1) and one job in queue
    #[tokio::test]
    async fn schedules_single_job() -> Result<(), TestError> {
        let test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let jobs = vec![WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000001,
        }];

        let result = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        redis.cleanup().await?;
        Ok(())
    }

    /// Tests scheduling multiple jobs.
    ///
    /// Verifies that the entity refresh tracker can schedule multiple worker jobs
    /// to the queue with staggered execution times and returns the correct count.
    ///
    /// Expected: Ok(3) and three jobs in queue
    #[tokio::test]
    async fn schedules_multiple_jobs() -> Result<(), TestError> {
        let test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let jobs = vec![
            WorkerJob::UpdateAllianceInfo {
                alliance_id: 99000001,
            },
            WorkerJob::UpdateAllianceInfo {
                alliance_id: 99000002,
            },
            WorkerJob::UpdateAllianceInfo {
                alliance_id: 99000003,
            },
        ];

        let result = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);

        redis.cleanup().await?;
        Ok(())
    }

    /// Tests scheduling with empty job list.
    ///
    /// Verifies that the entity refresh tracker correctly handles an empty job list
    /// by returning zero without errors or side effects.
    ///
    /// Expected: Ok(0)
    #[tokio::test]
    async fn returns_zero_for_empty_jobs() -> Result<(), TestError> {
        let test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let jobs = vec![];

        let result = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        redis.cleanup().await?;
        Ok(())
    }

    /// Tests duplicate job detection during scheduling.
    ///
    /// Verifies that the entity refresh tracker's duplicate detection prevents
    /// the same job from being added to the queue multiple times, even when
    /// included in the same batch.
    ///
    /// Expected: Ok(1) with only one job actually scheduled despite duplicates
    #[tokio::test]
    async fn handles_duplicate_jobs() -> Result<(), TestError> {
        let test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let jobs = vec![
            WorkerJob::UpdateAllianceInfo {
                alliance_id: 99000001,
            },
            WorkerJob::UpdateAllianceInfo {
                alliance_id: 99000001,
            }, // duplicate
        ];

        let result = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await;

        assert!(result.is_ok());
        // Only the first job is scheduled, duplicate is not pushed to queue
        assert_eq!(result.unwrap(), 1);

        redis.cleanup().await?;
        Ok(())
    }

    /// Tests scheduling a large batch of jobs.
    ///
    /// Verifies that the entity refresh tracker can efficiently handle scheduling
    /// many jobs (100 in this test) with appropriate time staggering to distribute
    /// load across the schedule interval.
    ///
    /// Expected: Ok(100) and 100 jobs in queue
    #[tokio::test]
    async fn schedules_many_jobs() -> Result<(), TestError> {
        let test = TestBuilder::new()
            .with_table(entity::prelude::EveFaction)
            .with_table(entity::prelude::EveAlliance)
            .build()
            .await?;
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let tracker = EntityRefreshTracker::new(
            &test.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let jobs: Vec<WorkerJob> = (1..=100)
            .map(|i| WorkerJob::UpdateAllianceInfo {
                alliance_id: 99000000 + i,
            })
            .collect();

        let result = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 100);

        redis.cleanup().await?;
        Ok(())
    }
}
