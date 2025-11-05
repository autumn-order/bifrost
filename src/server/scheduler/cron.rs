use crate::server::{
    model::worker::WorkerJob,
    scheduler::eve::{
        alliance::schedule_alliance_info_update, character::schedule_character_info_update,
        corporation::schedule_corporation_info_update,
    },
    service::eve::faction::FactionService,
};
use apalis_redis::RedisStorage;
use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};

use super::config::eve::alliance as alliance_config;
use super::config::eve::character as character_config;
use super::config::eve::corporation as corporation_config;
use super::config::eve::faction as faction_config;

macro_rules! add_cron_job {
    ($sched:expr, $cron:expr, $db:expr, $storage:expr, $fn:expr, $name:expr) => {{
        let db_clone = $db.clone();
        let storage_clone = $storage.clone();

        $sched
            .add(Job::new_async($cron, move |_, _| {
                let db = db_clone.clone();
                let mut job_storage = storage_clone.clone();

                Box::pin(async move {
                    match $fn(&db, &mut job_storage).await {
                        Ok(count) => tracing::info!("Scheduled {} {} update(s)", count, $name),
                        Err(e) => tracing::error!("Error scheduling {} update: {:?}", $name, e),
                    }
                })
            })?)
            .await?;
    }};
}

/// Initialize and start the cron job scheduler
pub async fn start_scheduler(
    db: &DatabaseConnection,
    esi_client: &eve_esi::Client,
    job_storage: &mut RedisStorage<WorkerJob>,
) -> Result<(), JobSchedulerError> {
    let sched = JobScheduler::new().await?;

    add_cron_job!(
        sched,
        alliance_config::CRON_EXPRESSION,
        db,
        job_storage,
        schedule_alliance_info_update,
        "alliance info"
    );

    add_cron_job!(
        sched,
        corporation_config::CRON_EXPRESSION,
        db,
        job_storage,
        schedule_corporation_info_update,
        "corporation info"
    );

    add_cron_job!(
        sched,
        character_config::CRON_EXPRESSION,
        db,
        job_storage,
        schedule_character_info_update,
        "character info"
    );

    let db_clone = db.clone();
    let esi_client_clone = esi_client.clone();

    sched
        .add(Job::new_async(
            faction_config::CRON_EXPRESSION,
            move |_, _| {
                let db = db_clone.clone();
                let esi_client = esi_client_clone.clone();

                Box::pin(async move {
                    let faction_service = FactionService::new(&db, &esi_client);

                    match faction_service.update_factions().await {
                        Ok(factions) => tracing::info!(
                            "Updated information for {} NPC factions",
                            factions.len()
                        ),
                        Err(e) => {
                            tracing::error!("Error updating NPC faction info: {:?}", e)
                        }
                    }
                })
            },
        )?)
        .await?;

    sched.start().await?;
    Ok(())
}
