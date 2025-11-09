use apalis_redis::RedisStorage;
use fred::prelude::*;
use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::{
        config::eve::character_affiliation::{CACHE_DURATION, SCHEDULE_INTERVAL},
        entity_refresh::{EntityRefreshTracker, SchedulableEntity},
    },
    util::eve::ESI_AFFILIATION_REQUEST_LIMIT,
};

pub struct CharacterAffiliation;

impl SchedulableEntity for CharacterAffiliation {
    type Entity = entity::eve_character::Entity;

    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::AffiliationUpdatedAt
    }

    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::Id
    }
}

/// Checks for character affiliation nearing expiration & schedules an update
pub async fn schedule_character_affiliation_update(
    db: &DatabaseConnection,
    redis_pool: &Pool,
    job_storage: &mut RedisStorage<WorkerJob>,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker =
        EntityRefreshTracker::new(db, redis_pool, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find characters that need affiliation updates
    let characters_needing_update = refresh_tracker
        .find_entries_needing_update::<CharacterAffiliation>()
        .await?;

    if characters_needing_update.is_empty() {
        return Ok(0);
    }

    // Extract character IDs from the models
    let character_ids: Vec<i64> = characters_needing_update
        .into_iter()
        .map(|character| character.character_id)
        .collect();

    // Divide character IDs into batches that respect ESI affiliation request limit of 1000
    let jobs: Vec<WorkerJob> = character_ids
        .chunks(ESI_AFFILIATION_REQUEST_LIMIT)
        .map(|chunk| WorkerJob::UpdateAffiliations {
            character_ids: chunk.to_vec(),
        })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<CharacterAffiliation>(job_storage, jobs)
        .await?;

    Ok(scheduled_job_count)
}
