//! Character information refresh scheduling.
//!
//! This module schedules updates for EVE Online character data. It queries the database for
//! characters whose cached information has expired (based on a 30-day cache duration) and
//! schedules staggered worker jobs to refresh their data from ESI. Character information
//! includes basic metadata like name, birthday, and description which rarely changes.

use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::{
        config::eve::character::{CACHE_DURATION, SCHEDULE_INTERVAL},
        entity_refresh::{EntityRefreshTracker, SchedulableEntity},
    },
    worker::queue::WorkerQueue,
};

/// Wrapper type for character entities participating in scheduled refreshes.
///
/// Implements `SchedulableEntity` to specify which columns track character updates and IDs,
/// enabling the generic refresh scheduling system to work with character data.
pub struct CharacterInfo;

impl SchedulableEntity for CharacterInfo {
    type Entity = entity::eve_character::Entity;

    /// Returns the `InfoUpdatedAt` column that tracks when character data was last refreshed.
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::InfoUpdatedAt
    }

    /// Returns the `CharacterId` column containing the EVE character ID.
    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::CharacterId
    }
}

/// Schedules character information refresh jobs for characters with expired cache data.
///
/// Queries the database for characters whose `info_updated_at` timestamp is older than the
/// cache expiration threshold (30 days), calculates an appropriate batch size to spread
/// updates across the cache period, and schedules worker jobs with staggered execution times
/// to distribute API load evenly. Character info updates refresh basic metadata that rarely
/// changes, so a long cache duration is used.
///
/// # Arguments
/// - `db` - Database connection for querying characters needing updates
/// - `worker_queue` - Worker queue for dispatching character refresh jobs
///
/// # Returns
/// - `Ok(usize)` - Number of character refresh jobs successfully scheduled (excludes duplicates)
/// - `Err(Error)` - Database query failed or job scheduling failed
pub async fn schedule_character_info_update(
    db: DatabaseConnection,
    worker_queue: WorkerQueue,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker = EntityRefreshTracker::new(&db, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find characters that need updating (returns character_ids)
    let character_ids = refresh_tracker
        .find_entries_needing_update::<CharacterInfo>()
        .await?;

    if character_ids.is_empty() {
        return Ok(0);
    }

    // Create and schedule jobs
    let jobs: Vec<WorkerJob> = character_ids
        .into_iter()
        .map(|character_id| WorkerJob::UpdateCharacterInfo { character_id })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<CharacterInfo>(&worker_queue, jobs)
        .await?;

    Ok(scheduled_job_count)
}
