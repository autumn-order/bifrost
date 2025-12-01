use sea_orm::DatabaseConnection;

use crate::server::{error::Error, model::worker::WorkerJob, worker::WorkerQueue};

/// Pushes an ESI NPC faction info update check to job queue
///
/// This doesn't actually execute a faction update every time, the job checks the
/// database if the factions we have stored were last updated within the 24 hour ESI
/// faction cache window.
///
/// If the factions are out of date, then an ESI fetch will be made and the factions in
/// database updated.
pub async fn schedule_faction_info_update(
    _db: DatabaseConnection,
    worker_queue: WorkerQueue,
) -> Result<usize, Error> {
    worker_queue.push(WorkerJob::UpdateFactionInfo {}).await?;

    const FACTION_UPDATES_SCHEDULED: usize = 1;

    Ok(FACTION_UPDATES_SCHEDULED)
}
