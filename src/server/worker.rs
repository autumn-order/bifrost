use apalis::prelude::Data;
use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;

use crate::server::{error::Error, model::worker::WorkerJob};

pub async fn handle_job(
    job: WorkerJob,
    db: Data<DatabaseConnection>,
    esi_client: Data<eve_esi::Client>,
) -> Result<(), Error> {
    match job {
        WorkerJob::UpdateAlliance { alliance_id } => {}
    }

    Ok(())
}
