use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub esi_client: eve_esi::Client,
}

// Used for `bifrost_test_utils` impl for the `state()` method to convert TestAppState to AppState
impl From<(DatabaseConnection, eve_esi::Client)> for AppState {
    fn from((db, esi_client): (DatabaseConnection, eve_esi::Client)) -> Self {
        AppState { db, esi_client }
    }
}
