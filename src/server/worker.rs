use apalis::prelude::Data;
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
        WorkerJob::UpdateAlliance { alliance_id } => {
            AllianceService::new(&db, &esi_client)
                .upsert_alliance(alliance_id)
                .await?;
        }
    }

    Ok(())
}
