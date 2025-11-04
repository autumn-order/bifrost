use apalis::prelude::*;
use apalis_redis::RedisStorage;
use chrono::{Duration, Utc};
use migration::Expr;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};

use crate::server::{
    model::worker::WorkerJob,
    util::task::{create_job_schedule, max_update_batch_size},
};

/// Cache ESI alliance information for 1 day
static ALLIANCE_INFO_CACHE: Duration = Duration::hours(24);
/// Interval the schedule cron task is ran
static SCHEDULE_INTERVAL: Duration = Duration::hours(3);

/// Checks for alliance information nearing expiration & schedules an update
// We can't test this function because apalis requires an actual redis instance
// and doesn't yet have a proper sqlite implementation for testing purposes.
pub async fn schedule_alliance_updates(
    db: &DatabaseConnection,
    job_storage: &mut RedisStorage<WorkerJob>,
) -> Result<usize, crate::server::error::Error> {
    let now = Utc::now().naive_utc();
    let cache_expiry_threshold = now - ALLIANCE_INFO_CACHE;
    let stale_job_threshold = now - (SCHEDULE_INTERVAL * 2);

    // Find alliances that need updating
    let alliances_needing_update =
        find_alliances_needing_update(db, cache_expiry_threshold, stale_job_threshold).await?;

    if alliances_needing_update.is_empty() {
        return Ok(0);
    }

    let alliance_ids: Vec<i64> = alliances_needing_update
        .iter()
        .map(|a| a.alliance_id)
        .collect();

    // Create and schedule jobs
    let jobs: Vec<WorkerJob> = alliances_needing_update
        .into_iter()
        .map(|alliance| WorkerJob::UpdateAllianceInfo {
            alliance_id: alliance.alliance_id,
        })
        .collect();

    let job_schedule = create_job_schedule(jobs, SCHEDULE_INTERVAL).await?;

    // Schedule all jobs to Redis first - if this fails, we won't mark the database
    // This prevents a race condition where DB is marked but jobs aren't actually scheduled
    for (job, scheduled_at) in job_schedule {
        job_storage.schedule(job, scheduled_at).await?;
    }

    // Only mark alliances as scheduled after ALL jobs are successfully queued
    mark_jobs_as_scheduled(db, &alliance_ids, now).await?;

    Ok(alliance_ids.len())
}

/// Finds alliances that need their information updated
async fn find_alliances_needing_update(
    db: &DatabaseConnection,
    cache_expiry_threshold: chrono::NaiveDateTime,
    stale_job_threshold: chrono::NaiveDateTime,
) -> Result<Vec<entity::eve_alliance::Model>, crate::server::error::Error> {
    let table_entries = entity::prelude::EveAlliance::find().count(db).await?;
    if table_entries == 0 {
        return Ok(Vec::new());
    }

    let max_batch_size =
        max_update_batch_size(table_entries, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

    let alliances = entity::prelude::EveAlliance::find()
        // Only update alliances after their cache has expired to get fresh data
        .filter(entity::eve_alliance::Column::UpdatedAt.lt(cache_expiry_threshold))
        // Reschedule if job wasn't completed or queue was lost
        .filter(
            entity::eve_alliance::Column::JobScheduledAt
                .is_null()
                .or(entity::eve_alliance::Column::JobScheduledAt.lt(stale_job_threshold)),
        )
        .order_by_asc(entity::eve_alliance::Column::UpdatedAt)
        .limit(max_batch_size)
        .all(db)
        .await?;

    Ok(alliances)
}

/// Marks alliances as having update jobs scheduled
async fn mark_jobs_as_scheduled(
    db: &DatabaseConnection,
    alliance_ids: &[i64],
    scheduled_at: chrono::NaiveDateTime,
) -> Result<(), crate::server::error::Error> {
    entity::prelude::EveAlliance::update_many()
        .col_expr(
            entity::eve_alliance::Column::JobScheduledAt,
            Expr::value(scheduled_at),
        )
        .filter(entity::eve_alliance::Column::Id.is_in(alliance_ids.to_vec()))
        .exec(db)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bifrost_test_utils::prelude::*;
    use chrono::Duration;

    mod find_alliances_needing_update {
        use super::*;

        /// Expect empty Vec when no alliances exist in the database
        #[tokio::test]
        async fn returns_empty_when_no_alliances() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let now = Utc::now().naive_utc();
            let cache_expiry = now - ALLIANCE_INFO_CACHE;
            let stale_job = now - (SCHEDULE_INTERVAL * 2);

            let result =
                find_alliances_needing_update(&test.state.db, cache_expiry, stale_job).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert!(alliances.is_empty());

            Ok(())
        }

        /// Expect empty Vec when all alliances are up to date
        #[tokio::test]
        async fn returns_empty_when_all_up_to_date() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            // Insert alliance with recent updated_at
            test.eve().insert_mock_alliance(1, None).await?;

            let now = Utc::now().naive_utc();
            let cache_expiry = now - ALLIANCE_INFO_CACHE;
            let stale_job = now - (SCHEDULE_INTERVAL * 2);

            let result =
                find_alliances_needing_update(&test.state.db, cache_expiry, stale_job).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert!(alliances.is_empty());

            Ok(())
        }

        /// Expect alliances with expired cache to be returned
        #[tokio::test]
        async fn returns_alliances_with_expired_cache() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance = test.eve().insert_mock_alliance(1, None).await?;

            // Update the alliance to have an old updated_at timestamp
            let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
            entity::prelude::EveAlliance::update_many()
                .col_expr(
                    entity::eve_alliance::Column::UpdatedAt,
                    Expr::value(old_timestamp),
                )
                .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
                .exec(&test.state.db)
                .await?;

            let now = Utc::now().naive_utc();
            let cache_expiry = now - ALLIANCE_INFO_CACHE;
            let stale_job = now - (SCHEDULE_INTERVAL * 2);

            let result =
                find_alliances_needing_update(&test.state.db, cache_expiry, stale_job).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 1);
            assert_eq!(alliances[0].id, alliance.id);

            Ok(())
        }

        /// Expect alliances with null job_scheduled_at to be returned
        #[tokio::test]
        async fn returns_alliances_with_null_job_scheduled() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance = test.eve().insert_mock_alliance(1, None).await?;

            // Update alliance with old updated_at and null job_scheduled_at
            let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
            entity::prelude::EveAlliance::update_many()
                .col_expr(
                    entity::eve_alliance::Column::UpdatedAt,
                    Expr::value(old_timestamp),
                )
                .col_expr(
                    entity::eve_alliance::Column::JobScheduledAt,
                    Expr::value(Option::<chrono::NaiveDateTime>::None),
                )
                .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
                .exec(&test.state.db)
                .await?;

            let now = Utc::now().naive_utc();
            let cache_expiry = now - ALLIANCE_INFO_CACHE;
            let stale_job = now - (SCHEDULE_INTERVAL * 2);

            let result =
                find_alliances_needing_update(&test.state.db, cache_expiry, stale_job).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 1);
            assert_eq!(alliances[0].id, alliance.id);

            Ok(())
        }

        /// Expect alliances with stale job_scheduled_at to be returned (rescheduled)
        #[tokio::test]
        async fn returns_alliances_with_stale_job() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance = test.eve().insert_mock_alliance(1, None).await?;

            // Update alliance with old updated_at and very old job_scheduled_at
            let old_updated = Utc::now().naive_utc() - Duration::hours(25);
            let old_job_scheduled = Utc::now().naive_utc() - Duration::hours(7);

            entity::prelude::EveAlliance::update_many()
                .col_expr(
                    entity::eve_alliance::Column::UpdatedAt,
                    Expr::value(old_updated),
                )
                .col_expr(
                    entity::eve_alliance::Column::JobScheduledAt,
                    Expr::value(old_job_scheduled),
                )
                .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
                .exec(&test.state.db)
                .await?;

            let now = Utc::now().naive_utc();
            let cache_expiry = now - ALLIANCE_INFO_CACHE;
            let stale_job = now - (SCHEDULE_INTERVAL * 2);

            let result =
                find_alliances_needing_update(&test.state.db, cache_expiry, stale_job).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 1);
            assert_eq!(alliances[0].id, alliance.id);

            Ok(())
        }

        /// Expect recently scheduled jobs to be excluded
        #[tokio::test]
        async fn excludes_recently_scheduled_jobs() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance = test.eve().insert_mock_alliance(1, None).await?;

            // Update alliance with old updated_at but recent job_scheduled_at
            let old_updated = Utc::now().naive_utc() - Duration::hours(25);
            let recent_job_scheduled = Utc::now().naive_utc() - Duration::minutes(5);

            entity::prelude::EveAlliance::update_many()
                .col_expr(
                    entity::eve_alliance::Column::UpdatedAt,
                    Expr::value(old_updated),
                )
                .col_expr(
                    entity::eve_alliance::Column::JobScheduledAt,
                    Expr::value(recent_job_scheduled),
                )
                .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
                .exec(&test.state.db)
                .await?;

            let now = Utc::now().naive_utc();
            let cache_expiry = now - ALLIANCE_INFO_CACHE;
            let stale_job = now - (SCHEDULE_INTERVAL * 2);

            let result =
                find_alliances_needing_update(&test.state.db, cache_expiry, stale_job).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert!(alliances.is_empty());

            Ok(())
        }

        /// Expect alliances ordered by oldest updated_at first
        #[tokio::test]
        async fn returns_oldest_updated_first() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
            let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

            // Set different updated_at timestamps
            let oldest = Utc::now().naive_utc() - Duration::hours(72);
            let middle = Utc::now().naive_utc() - Duration::hours(48);
            let newest = Utc::now().naive_utc() - Duration::hours(25);

            entity::prelude::EveAlliance::update_many()
                .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(middle))
                .filter(entity::eve_alliance::Column::Id.eq(alliance1.id))
                .exec(&test.state.db)
                .await?;

            entity::prelude::EveAlliance::update_many()
                .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(oldest))
                .filter(entity::eve_alliance::Column::Id.eq(alliance2.id))
                .exec(&test.state.db)
                .await?;

            entity::prelude::EveAlliance::update_many()
                .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(newest))
                .filter(entity::eve_alliance::Column::Id.eq(alliance3.id))
                .exec(&test.state.db)
                .await?;

            let now = Utc::now().naive_utc();
            let cache_expiry = now - ALLIANCE_INFO_CACHE;
            let stale_job = now - (SCHEDULE_INTERVAL * 2);

            let result =
                find_alliances_needing_update(&test.state.db, cache_expiry, stale_job).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 3);
            // Should be ordered: alliance2 (oldest), alliance1 (middle), alliance3 (newest)
            assert_eq!(alliances[0].id, alliance2.id);
            assert_eq!(alliances[1].id, alliance1.id);
            assert_eq!(alliances[2].id, alliance3.id);

            Ok(())
        }

        /// Expect Error when tables are missing
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let now = Utc::now().naive_utc();
            let cache_expiry = now - ALLIANCE_INFO_CACHE;
            let stale_job = now - (SCHEDULE_INTERVAL * 2);

            let result =
                find_alliances_needing_update(&test.state.db, cache_expiry, stale_job).await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod mark_jobs_as_scheduled {
        use super::*;

        /// Expect Ok when marking single alliance as scheduled
        #[tokio::test]
        async fn marks_single_alliance_as_scheduled() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance = test.eve().insert_mock_alliance(1, None).await?;
            let scheduled_time = Utc::now().naive_utc();

            let result =
                mark_jobs_as_scheduled(&test.state.db, &[alliance.alliance_id], scheduled_time)
                    .await;

            assert!(result.is_ok());

            // Verify the alliance was updated
            let updated_alliance = entity::prelude::EveAlliance::find_by_id(alliance.id)
                .one(&test.state.db)
                .await?
                .unwrap();

            assert!(updated_alliance.job_scheduled_at.is_some());
            let scheduled_at = updated_alliance.job_scheduled_at.unwrap();
            // Allow for small time differences in test execution
            assert!((scheduled_at - scheduled_time).num_seconds().abs() < 2);

            Ok(())
        }

        /// Expect Ok when marking multiple alliances as scheduled
        #[tokio::test]
        async fn marks_multiple_alliances_as_scheduled() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
            let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

            let scheduled_time = Utc::now().naive_utc();
            let alliance_ids = vec![
                alliance1.alliance_id,
                alliance2.alliance_id,
                alliance3.alliance_id,
            ];

            let result =
                mark_jobs_as_scheduled(&test.state.db, &alliance_ids, scheduled_time).await;

            assert!(result.is_ok());

            // Verify all alliances were updated
            for id in [alliance1.id, alliance2.id, alliance3.id] {
                let updated_alliance = entity::prelude::EveAlliance::find_by_id(id)
                    .one(&test.state.db)
                    .await?
                    .unwrap();

                assert!(updated_alliance.job_scheduled_at.is_some());
            }

            Ok(())
        }

        /// Expect Ok when marking empty list (no-op)
        #[tokio::test]
        async fn handles_empty_alliance_list() -> Result<(), TestError> {
            let test = test_setup_with_tables!(entity::prelude::EveAlliance)?;

            let scheduled_time = Utc::now().naive_utc();

            let result = mark_jobs_as_scheduled(&test.state.db, &[], scheduled_time).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect only specified alliances to be updated
        #[tokio::test]
        async fn only_updates_specified_alliances() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
            let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

            let scheduled_time = Utc::now().naive_utc();

            // Only mark alliance1 and alliance3
            let result = mark_jobs_as_scheduled(
                &test.state.db,
                &[alliance1.alliance_id, alliance3.alliance_id],
                scheduled_time,
            )
            .await;

            assert!(result.is_ok());

            // Verify alliance1 and alliance3 were updated
            let updated1 = entity::prelude::EveAlliance::find_by_id(alliance1.id)
                .one(&test.state.db)
                .await?
                .unwrap();
            assert!(updated1.job_scheduled_at.is_some());

            let updated3 = entity::prelude::EveAlliance::find_by_id(alliance3.id)
                .one(&test.state.db)
                .await?
                .unwrap();
            assert!(updated3.job_scheduled_at.is_some());

            // Verify alliance2 was NOT updated
            let updated2 = entity::prelude::EveAlliance::find_by_id(alliance2.id)
                .one(&test.state.db)
                .await?
                .unwrap();
            assert!(updated2.job_scheduled_at.is_none());

            Ok(())
        }

        /// Expect Ok when updating previously scheduled alliances
        #[tokio::test]
        async fn updates_previously_scheduled_alliances() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance = test.eve().insert_mock_alliance(1, None).await?;

            // Set initial job_scheduled_at
            let initial_time = Utc::now().naive_utc() - Duration::hours(1);
            entity::prelude::EveAlliance::update_many()
                .col_expr(
                    entity::eve_alliance::Column::JobScheduledAt,
                    Expr::value(initial_time),
                )
                .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
                .exec(&test.state.db)
                .await?;

            // Update with new scheduled time
            let new_scheduled_time = Utc::now().naive_utc();
            let result =
                mark_jobs_as_scheduled(&test.state.db, &[alliance.alliance_id], new_scheduled_time)
                    .await;

            assert!(result.is_ok());

            // Verify the time was updated
            let updated_alliance = entity::prelude::EveAlliance::find_by_id(alliance.id)
                .one(&test.state.db)
                .await?
                .unwrap();

            let scheduled_at = updated_alliance.job_scheduled_at.unwrap();
            assert!((scheduled_at - new_scheduled_time).num_seconds().abs() < 2);
            assert!((scheduled_at - initial_time).num_seconds() > 3500); // Should be ~1 hour apart

            Ok(())
        }

        /// Expect Error when tables are missing
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let scheduled_time = Utc::now().naive_utc();

            let result = mark_jobs_as_scheduled(&test.state.db, &[1, 2, 3], scheduled_time).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
