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

pub struct CharacterAffiliation;

impl SchedulableEntity for CharacterAffiliation {
    type Entity = entity::eve_character::Entity;

    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::AffiliationUpdatedAt
    }

    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::CharacterId
    }
}

/// Checks for character affiliation nearing expiration & schedules an update
pub async fn schedule_character_affiliation_update(
    db: &DatabaseConnection,
    worker_queue: &WorkerQueue,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker = EntityRefreshTracker::new(db, CACHE_DURATION, SCHEDULE_INTERVAL);

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
