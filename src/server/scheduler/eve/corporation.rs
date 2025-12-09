//! Corporation information refresh scheduling.
//!
//! This module schedules updates for EVE Online corporation data. It queries the database for
//! corporations whose cached information has expired (based on a 24-hour cache duration) and
//! schedules staggered worker jobs to refresh their data from ESI. Batch sizing ensures
//! all corporations are updated across the cache period without overwhelming the API or worker queue.

use sea_orm::{ColumnTrait, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::{
        config::eve::corporation::{CACHE_DURATION, SCHEDULE_INTERVAL},
        entity_refresh::{EntityRefreshTracker, SchedulableEntity},
        SchedulerState,
    },
};

/// Wrapper type for corporation entities participating in scheduled refreshes.
///
/// Implements `SchedulableEntity` to specify which columns track corporation updates and IDs,
/// enabling the generic refresh scheduling system to work with corporation data.
pub struct CorporationInfo;

impl SchedulableEntity for CorporationInfo {
    type Entity = entity::eve_corporation::Entity;

    /// Returns the `InfoUpdatedAt` column that tracks when corporation data was last refreshed.
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_corporation::Column::InfoUpdatedAt
    }

    /// Returns the `CorporationId` column containing the EVE corporation ID.
    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_corporation::Column::CorporationId
    }
}

/// Schedules corporation information refresh jobs for corporations with expired cache data.
///
/// Queries the database for corporations whose `info_updated_at` timestamp is older than the
/// cache expiration threshold (24 hours), calculates an appropriate batch size to spread
/// updates across the cache period, and schedules worker jobs with staggered execution times
/// to distribute API load evenly.
///
/// # Arguments
/// - `state` - Scheduler state containing database connection and worker queue for querying
///   corporations needing updates and dispatching refresh jobs
///
/// # Returns
/// - `Ok(usize)` - Number of corporation refresh jobs successfully scheduled (excludes duplicates)
/// - `Err(Error)` - Database query failed or job scheduling failed
pub async fn schedule_corporation_info_update(
    state: SchedulerState,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker = EntityRefreshTracker::new(&state, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find corporations that need updating (returns corporation_ids)
    let corporation_ids = refresh_tracker
        .find_entries_needing_update::<CorporationInfo>()
        .await?;

    if corporation_ids.is_empty() {
        return Ok(0);
    }

    // Create and schedule jobs
    let jobs: Vec<WorkerJob> = corporation_ids
        .into_iter()
        .map(|corporation_id| WorkerJob::UpdateCorporationInfo { corporation_id })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<CorporationInfo>(&state.queue, jobs)
        .await?;

    Ok(scheduled_job_count)
}
