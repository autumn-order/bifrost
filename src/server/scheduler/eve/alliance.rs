use apalis_redis::RedisStorage;
use fred::prelude::*;
use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::config::eve::alliance::{CACHE_DURATION, SCHEDULE_INTERVAL},
    scheduler::entity_refresh::{EntityRefreshTracker, SchedulableEntity},
};

/// Checks for alliance information nearing expiration & schedules an update
pub async fn schedule_alliance_info_update(
    db: &DatabaseConnection,
    redis_pool: &Pool,
    job_storage: &mut RedisStorage<WorkerJob>,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker =
        EntityRefreshTracker::new(db, redis_pool, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find alliances that need updating
    let alliances_needing_update = refresh_tracker
        .find_entries_needing_update::<entity::prelude::EveAlliance>()
        .await?;

    if alliances_needing_update.is_empty() {
        return Ok(0);
    }

    // Create and schedule jobs
    let jobs: Vec<(i32, WorkerJob)> = alliances_needing_update
        .into_iter()
        .map(|alliance| {
            (
                alliance.id,
                WorkerJob::UpdateAllianceInfo {
                    // Provide EVE alliance ID for ESI request, not the database entry ID
                    alliance_id: alliance.alliance_id,
                },
            )
        })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<entity::prelude::EveAlliance>(job_storage, jobs)
        .await?;

    Ok(scheduled_job_count)
}

impl SchedulableEntity for entity::eve_alliance::Entity {
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_alliance::Column::UpdatedAt
    }

    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_alliance::Column::Id
    }
}
