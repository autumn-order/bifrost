use apalis_redis::RedisStorage;
use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::config::eve::corporation::{CACHE_DURATION, SCHEDULE_INTERVAL},
    scheduler::entity_refresh::{EntityRefreshTracker, SchedulableEntity},
};

/// Checks for corporation information nearing expiration & schedules an update
pub async fn schedule_corporation_info_update(
    db: &DatabaseConnection,
    job_storage: &mut RedisStorage<WorkerJob>,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker = EntityRefreshTracker::new(db, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find corporations that need updating
    let corporations_needing_update = refresh_tracker
        .find_entries_needing_update::<entity::prelude::EveCorporation>()
        .await?;

    if corporations_needing_update.is_empty() {
        return Ok(0);
    }

    // Create and schedule jobs
    let jobs: Vec<(i32, WorkerJob)> = corporations_needing_update
        .into_iter()
        .map(|corporation| {
            (
                corporation.id,
                WorkerJob::UpdateCorporationInfo {
                    // Provide EVE corporation ID for ESI request
                    corporation_id: corporation.corporation_id,
                },
            )
        })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<entity::prelude::EveCorporation>(job_storage, jobs)
        .await?;

    Ok(scheduled_job_count)
}

impl SchedulableEntity for entity::eve_corporation::Entity {
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_corporation::Column::UpdatedAt
    }

    fn job_scheduled_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_corporation::Column::JobScheduledAt
    }

    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_corporation::Column::Id
    }
}
