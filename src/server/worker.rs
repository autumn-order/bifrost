use apalis::prelude::Data;
use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error, model::worker::WorkerJob, service::eve::alliance::AllianceService,
};

pub async fn handle_job(
    job: WorkerJob,
    db: Data<DatabaseConnection>,
    esi_client: Data<eve_esi::Client>,
) -> Result<(), Error> {
    match job {
        WorkerJob::UpdateAllianceInfo { alliance_id } => {
            tracing::debug!(
                "Processing alliance update for alliance_id: {}",
                alliance_id
            );

            AllianceService::new(&db, &esi_client)
                .upsert_alliance(alliance_id)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to update alliance {}: {:?}", alliance_id, e);
                    e
                })?;

            tracing::debug!("Successfully updated alliance {}", alliance_id);
        }
    }

    Ok(())
}
