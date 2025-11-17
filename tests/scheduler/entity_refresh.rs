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
use entity::prelude::{EveAlliance, EveFaction};
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
        entity::eve_alliance::Column::Id
    }
}

mod find_entries_needing_update {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_entries() -> Result<(), TestError> {
        let test = test_setup_with_tables!(EveFaction, EveAlliance)?;

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert!(entries.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn returns_empty_when_all_up_to_date() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
        test.eve().insert_mock_alliance(1, None).await?;

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert!(entries.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn returns_entries_with_expired_cache() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
        let alliance = test.eve().insert_mock_alliance(1, None).await?;

        // Update the alliance to have an old updated_at timestamp
        let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
        EveAlliance::update_many()
            .col_expr(
                entity::eve_alliance::Column::UpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
            .exec(&test.state.db)
            .await?;

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, alliance.id);

        Ok(())
    }

    #[tokio::test]
    async fn returns_oldest_updated_first() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
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
            .exec(&test.state.db)
            .await?;

        EveAlliance::update_many()
            .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(oldest))
            .filter(entity::eve_alliance::Column::Id.eq(alliance2.id))
            .exec(&test.state.db)
            .await?;

        EveAlliance::update_many()
            .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(newest))
            .filter(entity::eve_alliance::Column::Id.eq(alliance3.id))
            .exec(&test.state.db)
            .await?;

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 3);
        // Should be ordered: alliance2 (oldest), alliance1 (middle), alliance3 (newest)
        assert_eq!(entries[0].id, alliance2.id);
        assert_eq!(entries[1].id, alliance1.id);
        assert_eq!(entries[2].id, alliance3.id);

        Ok(())
    }

    #[tokio::test]
    async fn respects_batch_limit() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;

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
                .exec(&test.state.db)
                .await?;
        }

        // With 10 entries, cache 24h, interval 30min = 10 / 48 = 0 -> min 100
        // But we only have 10 entries, so we should get all 10
        let tracker = EntityRefreshTracker::new(
            &test.state.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let entries = result.unwrap();
        // Should return all 10 since that's less than MIN_BATCH_LIMIT (100)
        assert_eq!(entries.len(), 10);

        Ok(())
    }

    #[tokio::test]
    async fn handles_single_entry() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
        let alliance = test.eve().insert_mock_alliance(1, None).await?;

        let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
        EveAlliance::update_many()
            .col_expr(
                entity::eve_alliance::Column::UpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
            .exec(&test.state.db)
            .await?;

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
            alliance_config::CACHE_DURATION,
            alliance_config::SCHEDULE_INTERVAL,
        );

        let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, alliance.id);

        Ok(())
    }

    #[tokio::test]
    async fn fails_when_tables_missing() -> Result<(), TestError> {
        let test = test_setup_with_tables!()?;

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
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

    #[tokio::test]
    async fn schedules_single_job() -> Result<(), TestError> {
        let test = test_setup_with_tables!(EveFaction, EveAlliance)?;
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
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

    #[tokio::test]
    async fn schedules_multiple_jobs() -> Result<(), TestError> {
        let test = test_setup_with_tables!(EveFaction, EveAlliance)?;
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
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

    #[tokio::test]
    async fn returns_zero_for_empty_jobs() -> Result<(), TestError> {
        let test = test_setup_with_tables!(EveFaction, EveAlliance)?;
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
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

    #[tokio::test]
    async fn handles_duplicate_jobs() -> Result<(), TestError> {
        let test = test_setup_with_tables!(EveFaction, EveAlliance)?;
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
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
        // Both jobs scheduled with different timestamps (staggered execution)
        assert_eq!(result.unwrap(), 2);

        redis.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn schedules_many_jobs() -> Result<(), TestError> {
        let test = test_setup_with_tables!(EveFaction, EveAlliance)?;
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let tracker = EntityRefreshTracker::new(
            &test.state.db,
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
