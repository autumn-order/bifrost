use apalis::prelude::*;
use apalis_redis::RedisStorage;
use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, IntoSimpleExpr};

use crate::server::{
    model::worker::WorkerJob,
    util::task::{
        entity_refresh::{EntityRefreshTracker, SchedulableEntity},
        schedule::create_job_schedule,
    },
};

/// Cache ESI alliance information for 1 day
static ALLIANCE_INFO_CACHE: Duration = Duration::hours(24);
/// Interval the schedule cron task is ran
static SCHEDULE_INTERVAL: Duration = Duration::hours(3);

/// Checks for alliance information nearing expiration & schedules an update
pub async fn schedule_alliance_updates(
    db: &DatabaseConnection,
    job_storage: &mut RedisStorage<WorkerJob>,
) -> Result<usize, crate::server::error::Error> {
    let refresh_tracker = EntityRefreshTracker::new(db, ALLIANCE_INFO_CACHE, SCHEDULE_INTERVAL);

    // Find alliances that need updating
    let alliances_needing_update = refresh_tracker
        .find_entries_needing_update::<entity::prelude::EveAlliance>()
        .await?;

    if alliances_needing_update.is_empty() {
        return Ok(0);
    }

    let db_entry_ids: Vec<i32> = alliances_needing_update.iter().map(|a| a.id).collect();

    // Create and schedule jobs
    let jobs: Vec<WorkerJob> = alliances_needing_update
        .into_iter()
        .map(|alliance| WorkerJob::UpdateAllianceInfo {
            // Provide EVE alliance ID for ESI request, not the database entry ID
            alliance_id: alliance.alliance_id,
        })
        .collect();

    let job_schedule = create_job_schedule(jobs, SCHEDULE_INTERVAL).await?;

    // Schedule all jobs to Redis first - if this fails, we won't mark the database
    // This prevents a race condition where DB is marked but jobs aren't actually scheduled
    for (job, scheduled_at) in job_schedule {
        job_storage.schedule(job, scheduled_at).await?;
    }

    // Only mark alliances as scheduled after ALL jobs are successfully queued
    refresh_tracker
        .mark_jobs_as_scheduled::<entity::prelude::EveAlliance, i32>(
            &db_entry_ids,
            Utc::now().naive_local(),
        )
        .await?;

    Ok(db_entry_ids.len())
}

impl SchedulableEntity for entity::eve_alliance::Entity {
    fn updated_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_alliance::Column::UpdatedAt
    }

    fn job_scheduled_at_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_alliance::Column::JobScheduledAt
    }

    fn id_column() -> impl ColumnTrait + IntoSimpleExpr {
        entity::eve_alliance::Column::Id
    }
}
