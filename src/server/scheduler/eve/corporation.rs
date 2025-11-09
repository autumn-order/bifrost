use apalis_redis::RedisStorage;
use fred::prelude::*;
use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::config::eve::corporation::{CACHE_DURATION, SCHEDULE_INTERVAL},
    scheduler::entity_refresh::{EntityRefreshTracker, SchedulableEntity},
};

pub struct CorporationInfo;

impl SchedulableEntity for CorporationInfo {
    type Entity = entity::eve_corporation::Entity;

    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_corporation::Column::InfoUpdatedAt
    }

    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_corporation::Column::Id
    }
}

/// Checks for corporation information nearing expiration & schedules an update
pub async fn schedule_corporation_info_update(
    db: &DatabaseConnection,
    redis_pool: &Pool,
    job_storage: &mut RedisStorage<WorkerJob>,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker =
        EntityRefreshTracker::new(db, redis_pool, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find corporations that need updating
    let corporations_needing_update = refresh_tracker
        .find_entries_needing_update::<CorporationInfo>()
        .await?;

    if corporations_needing_update.is_empty() {
        return Ok(0);
    }

    // Create and schedule jobs
    let jobs: Vec<WorkerJob> = corporations_needing_update
        .into_iter()
        .map(|corporation| {
            WorkerJob::UpdateCorporationInfo {
                // Provide EVE corporation ID for ESI request
                corporation_id: corporation.corporation_id,
            }
        })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<CorporationInfo>(job_storage, jobs)
        .await?;

    Ok(scheduled_job_count)
}
