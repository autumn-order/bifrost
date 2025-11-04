use apalis_redis::RedisStorage;
use chrono::{DateTime, Duration, Utc};
use migration::{Expr, ExprTrait};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoSimpleExpr, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};

use crate::server::{
    error::Error,
    model::worker::WorkerJob,
    util::task::schedule::{create_job_schedule, max_schedule_batch_size},
};

/// Trait for entities that support scheduled cache updates
pub trait SchedulableEntity: EntityTrait {
    /// Get the column representing when the entity was last updated
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr;

    /// Get the column representing when a job was scheduled for this entity
    fn job_scheduled_at_column() -> impl ColumnTrait + IntoSimpleExpr;

    /// Get the ID column for this entity
    fn id_column() -> impl ColumnTrait + IntoSimpleExpr;
}

pub struct EntityRefreshTracker<'a> {
    db: &'a DatabaseConnection,
    cache_duration: Duration,
    schedule_interval: Duration,
}

impl<'a> EntityRefreshTracker<'a> {
    pub fn new(
        db: &'a DatabaseConnection,
        cache_duration: Duration,
        schedule_interval: Duration,
    ) -> Self {
        Self {
            db,
            cache_duration,
            schedule_interval,
        }
    }

    /// Finds entries that need their information updated
    pub async fn find_entries_needing_update<E>(
        &self,
    ) -> Result<Vec<E::Model>, crate::server::error::Error>
    where
        E: SchedulableEntity + Send + Sync,
        <E as EntityTrait>::Model: Send + Sync,
    {
        let table_entries = E::find().count(self.db).await?;
        if table_entries == 0 {
            return Ok(Vec::new());
        }

        let now = Utc::now().naive_utc();
        let cache_expiry_threshold = now - self.cache_duration;
        let stale_job_threshold = now - (self.schedule_interval * 2);

        let max_batch_size =
            max_schedule_batch_size(table_entries, self.cache_duration, self.schedule_interval);

        let entries = E::find()
            // Only update entries after their cache has expired to get fresh data
            .filter(E::updated_at_column().lt(cache_expiry_threshold))
            .filter(
                E::job_scheduled_at_column()
                    // Schedule if job isn't scheduled for entry
                    .is_null()
                    // Reschedule if job wasn't completed or queue was lost
                    .or(E::job_scheduled_at_column().lte(stale_job_threshold)),
            )
            .order_by_asc(E::updated_at_column())
            .limit(max_batch_size)
            .all(self.db)
            .await?;

        Ok(entries)
    }

    pub async fn schedule_jobs<E>(
        &self,
        job_storage: &mut RedisStorage<WorkerJob>,
        jobs: Vec<(i32, WorkerJob)>,
    ) -> Result<usize, Error>
    where
        E: SchedulableEntity + Send + Sync,
    {
        use apalis::prelude::Storage;

        let job_schedule = create_job_schedule(jobs, self.schedule_interval).await?;

        let mut scheduled_jobs = Vec::new();

        for (id, job, scheduled_at) in job_schedule {
            job_storage.schedule(job, scheduled_at.timestamp()).await?;

            scheduled_jobs.push((id, scheduled_at));
        }

        let scheduled_count = scheduled_jobs.len();
        self.mark_jobs_as_scheduled::<E>(scheduled_jobs).await?;

        Ok(scheduled_count)
    }

    /// Marks entries as having update jobs scheduled
    pub(self) async fn mark_jobs_as_scheduled<E>(
        &self,
        scheduled_jobs: Vec<(i32, DateTime<Utc>)>,
    ) -> Result<(), crate::server::error::Error>
    where
        E: SchedulableEntity + Send + Sync,
    {
        if scheduled_jobs.is_empty() {
            return Ok(());
        }

        let db_entry_ids: Vec<i32> = scheduled_jobs.iter().map(|(id, _)| *id).collect();

        // Build CASE WHEN id = x THEN timestamp_x ... END expression
        // Start with the first case, then chain the rest
        let mut scheduled_iter = scheduled_jobs.into_iter();
        let (first_id, first_scheduled_at) = scheduled_iter.next().unwrap(); // Safe because we checked is_empty

        let mut case_stmt = Expr::case(
            E::id_column().eq(first_id).into_simple_expr(),
            Expr::value(first_scheduled_at),
        );

        for (id, scheduled_at) in scheduled_iter {
            case_stmt = case_stmt.case(
                E::id_column().eq(id).into_simple_expr(),
                Expr::value(scheduled_at),
            );
        }

        E::update_many()
            .col_expr(E::job_scheduled_at_column(), case_stmt.into())
            .filter(E::id_column().is_in(db_entry_ids))
            .exec(self.db)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bifrost_test_utils::prelude::*;

    /// Cache duration for tests (24 hours)
    static ALLIANCE_INFO_CACHE: Duration = Duration::hours(24);
    /// Schedule interval for tests (3 hours)
    static SCHEDULE_INTERVAL: Duration = Duration::hours(3);

    mod find_entries_needing_update {
        use super::*;

        /// Expect empty Vec when no alliances exist in the database
        #[tokio::test]
        async fn returns_empty_when_no_entries() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .find_entries_needing_update::<entity::prelude::EveAlliance>()
                .await;

            assert!(result.is_ok());
            let entries = result.unwrap();
            assert!(entries.is_empty());

            Ok(())
        }

        /// Expect empty Vec when all entries are up to date
        #[tokio::test]
        async fn returns_empty_when_all_up_to_date() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            // Insert alliance with recent updated_at
            test.eve().insert_mock_alliance(1, None).await?;

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .find_entries_needing_update::<entity::prelude::EveAlliance>()
                .await;

            assert!(result.is_ok());
            let entries = result.unwrap();
            assert!(entries.is_empty());

            Ok(())
        }

        /// Expect entries with expired cache to be returned
        #[tokio::test]
        async fn returns_entries_with_expired_cache() -> Result<(), TestError> {
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

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .find_entries_needing_update::<entity::prelude::EveAlliance>()
                .await;

            assert!(result.is_ok());
            let entries = result.unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].id, alliance.id);

            Ok(())
        }

        /// Expect entries with null job_scheduled_at to be returned
        #[tokio::test]
        async fn returns_entries_with_null_job_scheduled() -> Result<(), TestError> {
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

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .find_entries_needing_update::<entity::prelude::EveAlliance>()
                .await;

            assert!(result.is_ok());
            let entries = result.unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].id, alliance.id);

            Ok(())
        }

        /// Expect entries with stale job_scheduled_at to be returned (rescheduled)
        #[tokio::test]
        async fn returns_entries_with_stale_job() -> Result<(), TestError> {
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

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .find_entries_needing_update::<entity::prelude::EveAlliance>()
                .await;

            assert!(result.is_ok());
            let entries = result.unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].id, alliance.id);

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

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .find_entries_needing_update::<entity::prelude::EveAlliance>()
                .await;

            assert!(result.is_ok());
            let entries = result.unwrap();
            assert!(entries.is_empty());

            Ok(())
        }

        /// Expect entries ordered by oldest updated_at first
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

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .find_entries_needing_update::<entity::prelude::EveAlliance>()
                .await;

            assert!(result.is_ok());
            let entries = result.unwrap();
            assert_eq!(entries.len(), 3);
            // Should be ordered: alliance2 (oldest), alliance1 (middle), alliance3 (newest)
            assert_eq!(entries[0].id, alliance2.id);
            assert_eq!(entries[1].id, alliance1.id);
            assert_eq!(entries[2].id, alliance3.id);

            Ok(())
        }

        /// Expect Error when tables are missing
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .find_entries_needing_update::<entity::prelude::EveAlliance>()
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod mark_jobs_as_scheduled {
        use super::*;

        /// Expect Ok when marking single entry as scheduled
        #[tokio::test]
        async fn marks_single_entry_as_scheduled() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance = test.eve().insert_mock_alliance(1, None).await?;
            let scheduled_time = Utc::now();

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .mark_jobs_as_scheduled::<entity::prelude::EveAlliance>(vec![(
                    alliance.id,
                    scheduled_time,
                )])
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
            assert!(
                (scheduled_at - scheduled_time.naive_utc())
                    .num_seconds()
                    .abs()
                    < 2
            );

            Ok(())
        }

        /// Expect Ok when marking multiple entries as scheduled
        #[tokio::test]
        async fn marks_multiple_entries_as_scheduled() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
            let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

            let scheduled_time = Utc::now();
            let scheuled_jobs = vec![
                (alliance1.id, scheduled_time),
                (alliance2.id, scheduled_time),
                (alliance3.id, scheduled_time),
            ];

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .mark_jobs_as_scheduled::<entity::prelude::EveAlliance>(scheuled_jobs)
                .await;

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
        async fn handles_empty_entry_list() -> Result<(), TestError> {
            let test = test_setup_with_tables!(entity::prelude::EveAlliance)?;

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .mark_jobs_as_scheduled::<entity::prelude::EveAlliance>(Vec::new())
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect only specified entries to be updated
        #[tokio::test]
        async fn only_updates_specified_entries() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
            let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

            let scheduled_time = Utc::now();
            let scheduled_jobs = vec![
                (alliance1.id, scheduled_time),
                (alliance3.id, scheduled_time),
            ];

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            // Only mark alliance1 and alliance3
            let result = scheduler
                .mark_jobs_as_scheduled::<entity::prelude::EveAlliance>(scheduled_jobs)
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

        /// Expect Ok when updating previously scheduled entries
        #[tokio::test]
        async fn updates_previously_scheduled_entries() -> Result<(), TestError> {
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

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            // Update with new scheduled time
            let new_scheduled_time = Utc::now();
            let result = scheduler
                .mark_jobs_as_scheduled::<entity::prelude::EveAlliance>(vec![(
                    alliance.id,
                    new_scheduled_time,
                )])
                .await;

            assert!(result.is_ok());

            // Verify the time was updated
            let updated_alliance = entity::prelude::EveAlliance::find_by_id(alliance.id)
                .one(&test.state.db)
                .await?
                .unwrap();

            let scheduled_at = updated_alliance.job_scheduled_at.unwrap();
            assert!(
                (scheduled_at - new_scheduled_time.naive_utc())
                    .num_seconds()
                    .abs()
                    < 2
            );
            assert!((scheduled_at - initial_time).num_seconds() > 3500); // Should be ~1 hour apart

            Ok(())
        }

        /// Expect Error when tables are missing
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let scheduled_time = Utc::now();

            let scheduler =
                EntityRefreshTracker::new(&test.state.db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

            let result = scheduler
                .mark_jobs_as_scheduled::<entity::prelude::EveAlliance>(vec![(1, scheduled_time)])
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
