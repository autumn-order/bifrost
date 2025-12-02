//! Character affiliation refresh scheduling.
//!
//! This module schedules updates for EVE Online character affiliation data. Affiliations
//! (corporation, alliance, faction) can change frequently as players join or leave organizations.
//! It queries the database for characters whose affiliation cache has expired (based on a 1-hour
//! cache duration) and schedules batch jobs to refresh multiple characters per ESI request,
//! respecting the 1000-character limit per affiliation API call.

use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::{
        config::eve::character_affiliation::{CACHE_DURATION, SCHEDULE_INTERVAL},
        entity_refresh::{EntityRefreshTracker, SchedulableEntity},
    },
    util::eve::ESI_AFFILIATION_REQUEST_LIMIT,
    worker::queue::WorkerQueue,
};

/// Wrapper type for character affiliation entities participating in scheduled refreshes.
///
/// Implements `SchedulableEntity` to specify which columns track affiliation updates and IDs,
/// enabling the generic refresh scheduling system to work with character affiliation data.
pub struct CharacterAffiliation;

impl SchedulableEntity for CharacterAffiliation {
    type Entity = entity::eve_character::Entity;

    /// Returns the `AffiliationUpdatedAt` column that tracks when affiliation data was last refreshed.
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::AffiliationUpdatedAt
    }

    /// Returns the `CharacterId` column containing the EVE character ID.
    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::CharacterId
    }
}

/// Schedules character affiliation refresh jobs for characters with expired cache data.
///
/// Queries the database for characters whose `affiliation_updated_at` timestamp is older than
/// the cache expiration threshold (1 hour), calculates an appropriate batch size, and schedules
/// worker jobs with staggered execution times. Unlike other entity types, affiliation updates
/// are batched into groups of up to 1000 character IDs per job to match ESI's bulk affiliation
/// endpoint limit, reducing API call overhead while keeping affiliation data fresh.
///
/// # Arguments
/// - `db` - Database connection for querying characters needing affiliation updates
/// - `worker_queue` - Worker queue for dispatching affiliation refresh jobs
///
/// # Returns
/// - `Ok(usize)` - Number of affiliation refresh jobs successfully scheduled (excludes duplicates)
/// - `Err(Error)` - Database query failed or job scheduling failed
pub async fn schedule_character_affiliation_update(
    db: DatabaseConnection,
    worker_queue: WorkerQueue,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker = EntityRefreshTracker::new(&db, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find characters that need affiliation updates (returns character_ids)
    let character_ids = refresh_tracker
        .find_entries_needing_update::<CharacterAffiliation>()
        .await?;

    if character_ids.is_empty() {
        return Ok(0);
    }

    // Divide character IDs into batches that respect ESI affiliation request limit of 1000
    let jobs: Vec<WorkerJob> = character_ids
        .chunks(ESI_AFFILIATION_REQUEST_LIMIT)
        .map(|chunk| WorkerJob::UpdateAffiliations {
            character_ids: chunk.to_vec(),
        })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<CharacterAffiliation>(&worker_queue, jobs)
        .await?;

    Ok(scheduled_job_count)
}
