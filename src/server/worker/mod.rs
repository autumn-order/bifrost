mod handler;
mod queue;

use apalis::prelude::Data;
use dioxus_logger::tracing;
use fred::prelude::*;
use sea_orm::DatabaseConnection;

use crate::server::{error::Error, model::worker::WorkerJob};

pub use handler::WorkerJobHandler;

pub async fn handle_job(
    job: WorkerJob,
    db: Data<DatabaseConnection>,
    esi_client: Data<eve_esi::Client>,
    redis_pool: Data<Pool>,
) -> Result<(), Error> {
    let handler = WorkerJobHandler::new(&db, &esi_client);

    // Execute the job
    let result = match &job {
        WorkerJob::UpdateAllianceInfo { alliance_id } => {
            handler.update_alliance_info(*alliance_id).await
        }
        WorkerJob::UpdateCorporationInfo { corporation_id } => {
            handler.update_corporation_info(*corporation_id).await
        }
        WorkerJob::UpdateCharacterInfo { character_id } => {
            handler.update_character_info(*character_id).await
        }
        WorkerJob::UpdateAffiliations { character_ids } => {
            handler.update_affiliations(character_ids.clone()).await
        }
    };

    // Clean up the tracking key after job completes (whether success or failure)
    let tracking_key = job.tracking_key();
    let delete_result: Result<(), fred::prelude::Error> = redis_pool.del(&tracking_key).await;

    if let Err(e) = delete_result {
        tracing::error!(
            "Failed to delete tracking key '{}' after job completion: {}",
            tracking_key,
            e
        );
    }

    result
}
