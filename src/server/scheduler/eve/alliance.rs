use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::{
        config::eve::alliance::{CACHE_DURATION, SCHEDULE_INTERVAL},
        entity_refresh::{EntityRefreshTracker, SchedulableEntity},
    },
    worker::queue::WorkerQueue,
};

pub struct AllianceInfo;

impl SchedulableEntity for AllianceInfo {
    type Entity = entity::eve_alliance::Entity;

    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_alliance::Column::UpdatedAt
    }

    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_alliance::Column::AllianceId
    }
}

/// Checks for alliance information nearing expiration & schedules an update
pub async fn schedule_alliance_info_update(
    db: DatabaseConnection,
    worker_queue: WorkerQueue,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker = EntityRefreshTracker::new(&db, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find alliances that need updating (returns alliance_ids)
    let alliance_ids = refresh_tracker
        .find_entries_needing_update::<AllianceInfo>()
        .await?;

    if alliance_ids.is_empty() {
        return Ok(0);
    }

    // Create and schedule jobs
    let jobs: Vec<WorkerJob> = alliance_ids
        .into_iter()
        .map(|alliance_id| WorkerJob::UpdateAllianceInfo { alliance_id })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<AllianceInfo>(&worker_queue, jobs)
        .await?;

    Ok(scheduled_job_count)
}
