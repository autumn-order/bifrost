//! Faction information refresh scheduling.
//!
//! This module schedules updates for EVE Online NPC faction data. Unlike other entity types,
//! factions use a simplified scheduling approach because there are only a small, fixed number
//! of NPC factions in EVE Online. Rather than querying for individual faction expiration times,
//! the scheduler simply enqueues a single job that checks and updates all factions if needed.

use crate::server::{error::Error, model::worker::WorkerJob, scheduler::SchedulerState};

/// Schedules a faction information update check to the worker queue.
///
/// Enqueues a single worker job that checks if cached faction data has expired and refreshes
/// it if necessary. Unlike other entity types (alliances, corporations, characters), factions
/// don't use batch scheduling because there are only a small, fixed number of NPC factions in
/// EVE Online (~25 factions total). The worker job itself determines if a refresh is needed
/// by comparing the stored `updated_at` timestamp against the 24-hour cache duration.
///
/// # Arguments
/// - `state` - Scheduler state containing the worker queue (database connection unused for factions)
///
/// # Returns
/// - `Ok(1)` - Successfully scheduled one faction update check job
/// - `Err(Error)` - Failed to enqueue the job to the worker queue
pub async fn schedule_faction_info_update(state: SchedulerState) -> Result<usize, Error> {
    let was_scheduled = state.queue.push(WorkerJob::UpdateFactionInfo {}).await?;

    let scheduled_count = if was_scheduled { 1 } else { 0 };

    Ok(scheduled_count)
}
