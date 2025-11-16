use chrono::{Duration, Utc};
use dioxus_logger::tracing;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoSimpleExpr, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};

use super::schedule::{calculate_batch_limit, create_job_schedule};
use crate::server::{error::Error, model::worker::WorkerJob, worker::queue::WorkerQueue};

/// Trait for entities that support scheduled cache updates
pub trait SchedulableEntity {
    /// The actual SeaORM entity type
    type Entity: EntityTrait;

    /// Get the column representing when the entity was last updated
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr;

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
    pub async fn find_entries_needing_update<S>(
        &self,
    ) -> Result<Vec<<S::Entity as EntityTrait>::Model>, crate::server::error::Error>
    where
        S: SchedulableEntity + Send + Sync,
        S::Entity: Send + Sync,
        <S::Entity as EntityTrait>::Model: Send + Sync,
    {
        let table_entries = S::Entity::find().count(self.db).await?;
        if table_entries == 0 {
            return Ok(Vec::new());
        }

        let now = Utc::now().naive_utc();
        let cache_expiry_threshold = now - self.cache_duration;

        let max_batch_size =
            calculate_batch_limit(table_entries, self.cache_duration, self.schedule_interval);

        let entries = S::Entity::find()
            // Only update entries after their cache has expired to get fresh data
            .filter(S::updated_at_column().lt(cache_expiry_threshold))
            .order_by_asc(S::updated_at_column())
            .limit(max_batch_size)
            .all(self.db)
            .await?;

        Ok(entries)
    }

    pub async fn schedule_jobs<S>(
        &self,
        worker_queue: &WorkerQueue,
        jobs: Vec<WorkerJob>,
    ) -> Result<usize, Error>
    where
        S: SchedulableEntity + Send + Sync,
    {
        let job_schedule = create_job_schedule(jobs, self.schedule_interval).await?;

        let mut scheduled_count = 0;

        // Try to schedule each job, checking for duplicates
        for (job, scheduled_at) in job_schedule {
            let ttl_seconds = (self.schedule_interval * 2).num_seconds();

            if ttl_seconds <= 0 {
                tracing::warn!("Invalid TTL calculated for job, skipping");
                continue;
            }

            match worker_queue.schedule(job, scheduled_at).await {
                Ok(_) => {
                    scheduled_count += 1;
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        Ok(scheduled_count)
    }
}

// Ignore all these tests for now due to redis dependency which is not
// properly implemented for testing.
/*
#[cfg(test)]
mod tests {
    use super::*;
    use bifrost_test_utils::prelude::*;

    /// Cache duration for tests (24 hours)
    static ALLIANCE_INFO_CACHE: Duration = Duration::hours(24);
    /// Schedule interval for tests (3 hours)
    static SCHEDULE_INTERVAL: Duration = Duration::hours(3);

    mod find_entries_needing_update {
        use migration::Expr;

        use super::*;

        /// Expect empty Vec when no alliances exist in the database
        #[tokio::test]
        async fn returns_empty_when_no_entries() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let redis_pool = test.redis_pool().await?;

            let scheduler = EntityRefreshTracker::new(
                &test.state.db,
                redis_pool,
                ALLIANCE_INFO_CACHE,
                SCHEDULE_INTERVAL,
            );

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
            let redis_pool = test.redis_pool().await?;

            let scheduler = EntityRefreshTracker::new(
                &test.state.db,
                redis_pool,
                ALLIANCE_INFO_CACHE,
                SCHEDULE_INTERVAL,
            );

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
            let redis_pool = test.redis_pool().await?;

            let scheduler = EntityRefreshTracker::new(
                &test.state.db,
                redis_pool,
                ALLIANCE_INFO_CACHE,
                SCHEDULE_INTERVAL,
            );

            let result = scheduler
                .find_entries_needing_update::<entity::prelude::EveAlliance>()
                .await;

            assert!(result.is_ok());
            let entries = result.unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].id, alliance.id);

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
            let redis_pool = test.redis_pool().await?;

            let scheduler = EntityRefreshTracker::new(
                &test.state.db,
                redis_pool,
                ALLIANCE_INFO_CACHE,
                SCHEDULE_INTERVAL,
            );

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
            let mut test = test_setup_with_tables!()?;
            let redis_pool = test.redis_pool().await?;

            let scheduler = EntityRefreshTracker::new(
                &test.state.db,
                redis_pool,
                ALLIANCE_INFO_CACHE,
                SCHEDULE_INTERVAL,
            );

            let result = scheduler
                .find_entries_needing_update::<entity::prelude::EveAlliance>()
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
*/
