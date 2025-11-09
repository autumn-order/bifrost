use apalis::prelude::Data;
use sea_orm::DatabaseConnection;

use crate::server::{error::Error, model::worker::WorkerJob};

mod handler;

pub use handler::WorkerJobHandler;

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
        WorkerJob::UpdateCorporationInfo { corporation_id } => {
            handler.update_corporation_info(corporation_id).await?
        }
        WorkerJob::UpdateCharacterInfo { character_id } => {
            handler.update_character_info(character_id).await?
        }
    }

    Ok(())
}
