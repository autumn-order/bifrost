use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    scheduler::{
        config::eve::corporation::{CACHE_DURATION, SCHEDULE_INTERVAL},
        entity_refresh::{EntityRefreshTracker, SchedulableEntity},
    },
    worker::queue::WorkerQueue,
};

pub struct CorporationInfo;

impl SchedulableEntity for CorporationInfo {
    type Entity = entity::eve_corporation::Entity;

    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_corporation::Column::InfoUpdatedAt
    }

    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_corporation::Column::CorporationId
    }
}

/// Checks for corporation information nearing expiration & schedules an update
pub async fn schedule_corporation_info_update(
    db: DatabaseConnection,
    worker_queue: WorkerQueue,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker = EntityRefreshTracker::new(&db, CACHE_DURATION, SCHEDULE_INTERVAL);

    // Find corporations that need updating (returns corporation_ids)
    let corporation_ids = refresh_tracker
        .find_entries_needing_update::<CorporationInfo>()
        .await?;

    if corporation_ids.is_empty() {
        return Ok(0);
    }

    // Create and schedule jobs
    let jobs: Vec<WorkerJob> = corporation_ids
        .into_iter()
        .map(|corporation_id| WorkerJob::UpdateCorporationInfo { corporation_id })
        .collect();

    let scheduled_job_count = refresh_tracker
        .schedule_jobs::<CorporationInfo>(&worker_queue, jobs)
        .await?;

    Ok(scheduled_job_count)
}
