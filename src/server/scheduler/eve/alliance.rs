//! Alliance information refresh scheduling.
//!
//! This module schedules updates for EVE Online alliance data. It queries the database for
//! alliances whose cached information has expired (based on a 24-hour cache duration) and
//! schedules staggered worker jobs to refresh their data from ESI. Batch sizing ensures
//! all alliances are updated across the cache period without overwhelming the API or worker queue.

use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::{
        config::eve::alliance::{CACHE_DURATION, SCHEDULE_INTERVAL},
        entity_refresh::{EntityRefreshTracker, SchedulableEntity},
    },
    worker::queue::WorkerQueue,
};

/// Wrapper type for alliance entities participating in scheduled refreshes.
///
/// Implements `SchedulableEntity` to specify which columns track alliance updates and IDs,
/// enabling the generic refresh scheduling system to work with alliance data.
pub struct AllianceInfo;

impl SchedulableEntity for AllianceInfo {
    type Entity = entity::eve_alliance::Entity;

    /// Returns the `UpdatedAt` column that tracks when alliance data was last refreshed.
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_alliance::Column::UpdatedAt
    }

    /// Returns the `AllianceId` column containing the EVE alliance ID.
    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_alliance::Column::AllianceId
    }
}

/// Schedules alliance information refresh jobs for alliances with expired cache data.
///
/// Queries the database for alliances whose `updated_at` timestamp is older than the cache
/// expiration threshold (24 hours), calculates an appropriate batch size to spread updates
/// across the cache period, and schedules worker jobs with staggered execution times to
/// distribute API load evenly.
///
/// # Arguments
/// - `db` - Database connection for querying alliances needing updates
/// - `worker_queue` - Worker queue for dispatching alliance refresh jobs
///
/// # Returns
/// - `Ok(usize)` - Number of alliance refresh jobs successfully scheduled (excludes duplicates)
/// - `Err(Error)` - Database query failed or job scheduling failed
pub async fn schedule_alliance_info_update(
    db: DatabaseConnection,
    worker_queue: WorkerQueue,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker = EntityRefreshTracker::new(&db, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find alliances that need updating (returns alliance_ids)
    let alliance_ids = refresh_tracker
        .find_entries_needing_update::<AllianceInfo>()
        .await?;

    if alliance_ids.is_empty() {
        return Ok(0);
    }

    // Create and schedule jobs
    let jobs: Vec<WorkerJob> = alliance_ids
        .into_iter()
        .map(|alliance_id| WorkerJob::UpdateAllianceInfo { alliance_id })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<AllianceInfo>(&worker_queue, jobs)
        .await?;

    Ok(scheduled_job_count)
}
