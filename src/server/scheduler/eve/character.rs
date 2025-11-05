use apalis_redis::RedisStorage;
use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::config::eve::character::{CACHE_DURATION, SCHEDULE_INTERVAL},
    util::task::entity_refresh::{EntityRefreshTracker, SchedulableEntity},
};

/// Checks for character information nearing expiration & schedules an update
pub async fn schedule_character_info_update(
    db: &DatabaseConnection,
    job_storage: &mut RedisStorage<WorkerJob>,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker = EntityRefreshTracker::new(db, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find characters that need updating
    let characters_needing_update = refresh_tracker
        .find_entries_needing_update::<entity::prelude::EveCharacter>()
        .await?;

    if characters_needing_update.is_empty() {
        return Ok(0);
    }

    // Create and schedule jobs
    let jobs: Vec<(i32, WorkerJob)> = characters_needing_update
        .into_iter()
        .map(|character| {
            (
                character.id,
                WorkerJob::UpdateCharacterInfo {
                    // Provide EVE character ID for ESI request
                    character_id: character.character_id,
                },
            )
        })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<entity::prelude::EveCharacter>(job_storage, jobs)
        .await?;

    Ok(scheduled_job_count)
}

impl SchedulableEntity for entity::eve_character::Entity {
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::UpdatedAt
    }

    fn job_scheduled_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::JobScheduledAt
    }

    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_character::Column::Id
    }
}
