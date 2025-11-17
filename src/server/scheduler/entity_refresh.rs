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

    /// Get the resource ID column for this entity (e.g., alliance_id, character_id)
    /// This should return the EVE entity ID, not the database primary key
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
    /// Returns a vector of resource IDs (e.g., alliance_id, character_id) for entries
    /// that have expired cache and need to be refreshed
    pub async fn find_entries_needing_update<S>(
        &self,
    ) -> Result<Vec<i64>, crate::server::error::Error>
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

        let ids: Vec<i64> = S::Entity::find()
            // Only update entries after their cache has expired to get fresh data
            .filter(S::updated_at_column().lt(cache_expiry_threshold))
            .order_by_asc(S::updated_at_column())
            .limit(max_batch_size)
            .select_only()
            .column(S::id_column())
            .into_tuple()
            .all(self.db)
            .await?;

        Ok(ids)
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
                Ok(was_scheduled) => {
                    if was_scheduled {
                        scheduled_count += 1;
                    }
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        Ok(scheduled_count)
    }
}
