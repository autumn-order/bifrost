use apalis::prelude::Data;
use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error, model::worker::WorkerJob, service::eve::alliance::AllianceService,
};

pub struct WorkerJobHandler<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

pub async fn handle_job(
    job: WorkerJob,
    db: Data<DatabaseConnection>,
    esi_client: Data<eve_esi::Client>,
) -> Result<(), Error> {
    let handler = WorkerJobHandler::new(&db, &esi_client);

    match job {
        WorkerJob::UpdateAllianceInfo { alliance_id } => {
            handler.update_alliance_info(alliance_id).await?
        }
    }

    Ok(())
}

impl<'a> WorkerJobHandler<'a> {
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    async fn update_alliance_info(&self, alliance_id: i64) -> Result<(), Error> {
        tracing::debug!(
            "Processing alliance update for alliance_id: {}",
            alliance_id
        );

        AllianceService::new(&self.db, &self.esi_client)
            .upsert_alliance(alliance_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update alliance {}: {:?}", alliance_id, e);
                e
            })?;

        tracing::debug!("Successfully updated alliance {}", alliance_id);

        Ok(())
    }
}
