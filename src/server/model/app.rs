use sea_orm::DatabaseConnection;

use crate::server::worker::Worker;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub esi_client: eve_esi::Client,
    pub worker: Worker,
}
