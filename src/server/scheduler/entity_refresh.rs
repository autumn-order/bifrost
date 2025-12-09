//! Entity refresh tracking and scheduling for cached EVE Online data.
//!
//! This module provides a generic system for tracking when cached entity data expires and
//! scheduling refresh jobs via the worker queue. The `SchedulableEntity` trait allows any
//! EVE entity type (alliances, corporations, characters, etc.) to participate in the
//! scheduled refresh system by specifying their update timestamp and ID columns.

use chrono::{Duration, Utc};
use dioxus_logger::tracing;
use sea_orm::{
    ColumnTrait, EntityTrait, IntoSimpleExpr, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};

use crate::server::{
    error::Error,
    model::worker::WorkerJob,
    scheduler::{
        schedule::{calculate_batch_limit, create_job_schedule},
        SchedulerState,
    },
    worker::queue::WorkerQueue,
};

/// Trait for entities that support scheduled cache updates.
///
/// Implementing this trait allows an entity type to participate in the automated refresh
/// scheduling system. The trait specifies which columns track update timestamps and entity IDs,
/// enabling the scheduler to query for expired entries and schedule appropriate refresh jobs.
pub trait SchedulableEntity {
    /// The SeaORM entity type this schedulable entity wraps.
    ///
    /// Must implement `EntityTrait` to support database queries for finding expired entries.
    type Entity: EntityTrait;

    /// Returns the column that tracks when this entity was last updated.
    ///
    /// The scheduler uses this column to identify entries whose cached data has expired
    /// by comparing against `(now - cache_duration)`.
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr;

    /// Returns the column containing the EVE entity ID (not the database primary key).
    ///
    /// This ID is used to construct worker jobs for refreshing specific entities.
    /// For example, `alliance_id` for alliances or `character_id` for characters.
    fn id_column() -> impl ColumnTrait + IntoSimpleExpr;
}

/// Tracks and schedules refresh jobs for entities with expiring cached data.
///
/// `EntityRefreshTracker` queries the database for entities whose cached data has expired
/// (based on `updated_at` timestamps), calculates appropriate batch sizes to spread updates
/// over time, and schedules worker jobs with staggered execution times to avoid overwhelming
/// the API or worker system.
pub struct EntityRefreshTracker<'a> {
    state: &'a SchedulerState,
    cache_duration: Duration,
    schedule_interval: Duration,
}

impl<'a> EntityRefreshTracker<'a> {
    /// Creates a new refresh tracker with the specified cache and scheduling parameters.
    ///
    /// The tracker uses these parameters to determine which entities need updates and how
    /// many to schedule per run to spread the refresh load evenly across the cache period.
    ///
    /// # Arguments
    /// - `db` - Database connection for querying entities
    /// - `cache_duration` - How long cached entity data remains valid before expiring
    /// - `schedule_interval` - How frequently the scheduler checks for expired entities
    ///
    /// # Returns
    /// A new `EntityRefreshTracker` instance configured with the provided parameters.
    pub fn new(
        state: &'a SchedulerState,
        cache_duration: Duration,
        schedule_interval: Duration,
    ) -> Self {
        Self {
            state,
            cache_duration,
            schedule_interval,
        }
    }

    /// Finds entity IDs that need their cached information refreshed.
    ///
    /// Queries the database for entities whose `updated_at` timestamp is older than the
    /// cache expiration threshold, orders them by staleness (oldest first), and limits
    /// the result to an appropriate batch size. The batch size is calculated to spread
    /// all entity updates evenly across the cache duration.
    ///
    /// # Arguments
    /// - `S` - The `SchedulableEntity` type to query for (e.g., `AllianceInfo`, `CharacterInfo`)
    ///
    /// # Returns
    /// - `Ok(Vec<i64>)` - Vector of EVE entity IDs (e.g., alliance_id, character_id) that need updates
    /// - `Err(Error)` - Database query failed
    ///
    /// # Example
    /// ```ignore
    /// let tracker = EntityRefreshTracker::new(&db, Duration::hours(24), Duration::minutes(30));
    /// let alliance_ids = tracker.find_entries_needing_update::<AllianceInfo>().await?;
    /// // Returns up to ~208 alliance IDs whose cache has expired
    /// ```
    pub async fn find_entries_needing_update<S>(
        &self,
    ) -> Result<Vec<i64>, crate::server::error::Error>
    where
        S: SchedulableEntity + Send + Sync,
        S::Entity: Send + Sync,
        <S::Entity as EntityTrait>::Model: Send + Sync,
    {
        let table_entries = S::Entity::find().count(&self.state.db).await?;
        if table_entries == 0 {
            return Ok(Vec::new());
        }

        let now = Utc::now().naive_utc();
        let cache_expiry_threshold = now - self.cache_duration;

        let max_batch_size = calculate_batch_limit(
            table_entries,
            self.cache_duration,
            self.schedule_interval,
            self.state.offset_for_esi_downtime,
        );

        let ids: Vec<i64> = S::Entity::find()
            // Only update entries after their cache has expired to get fresh data
            .filter(S::updated_at_column().lt(cache_expiry_threshold))
            .order_by_asc(S::updated_at_column())
            .limit(max_batch_size)
            .select_only()
            .column(S::id_column())
            .into_tuple()
            .all(&self.state.db)
            .await?;

        Ok(ids)
    }

    /// Schedules worker jobs with staggered execution times across the scheduling interval.
    ///
    /// Takes a list of worker jobs and schedules them to execute at evenly distributed times
    /// across the scheduling window. This spreads API load and worker queue pressure over time
    /// rather than executing all jobs immediately. Jobs are deduplicated at scheduling time,
    /// so only jobs that aren't already queued will be scheduled.
    ///
    /// # Arguments
    /// - `S` - The `SchedulableEntity` type (used for logging context)
    /// - `worker_queue` - Worker queue to schedule jobs on
    /// - `jobs` - Vector of worker jobs to schedule (e.g., `UpdateAllianceInfo`, `UpdateCharacterInfo`)
    ///
    /// # Returns
    /// - `Ok(usize)` - Number of jobs successfully scheduled (excludes duplicates)
    /// - `Err(Error)` - Failed to schedule jobs or calculate schedule
    ///
    /// # Example
    /// ```ignore
    /// let jobs = vec![WorkerJob::UpdateAllianceInfo { alliance_id: 1 }, /* ... */];
    /// let count = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await?;
    /// // Returns number of jobs actually scheduled (may be less than jobs.len() due to deduplication)
    /// ```
    pub async fn schedule_jobs<S>(
        &self,
        worker_queue: &WorkerQueue,
        jobs: Vec<WorkerJob>,
    ) -> Result<usize, Error>
    where
        S: SchedulableEntity + Send + Sync,
    {
        let job_schedule = create_job_schedule(
            jobs,
            self.schedule_interval,
            self.state.offset_for_esi_downtime,
        )
        .await?;

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
                    return Err(e);
                }
            }
        }

        Ok(scheduled_count)
    }
}
