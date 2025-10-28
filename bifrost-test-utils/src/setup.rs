use std::sync::Arc;

use bifrost::server::{error::Error, model::app::AppState};
use mockito::{Server, ServerGuard};
use sea_orm::Database;
use tower_sessions::{MemoryStore, Session};

use crate::constant::{
    TEST_CALLBACK_URL, TEST_ESI_CLIENT_ID, TEST_ESI_CLIENT_SECRET, TEST_USER_AGENT,
};

pub struct TestSetup {
    pub server: ServerGuard,
    pub state: AppState,
    pub session: Session,
}

impl TestSetup {
    pub async fn new() -> Result<Self, Error> {
        let mock_server = Server::new_async().await;
        let mock_server_url = mock_server.url();

        let esi_config = eve_esi::Config::builder()
            .esi_url(&mock_server_url)
            .token_url(&format!("{}/v2/oauth/token", mock_server.url()))
            .jwk_url(&format!("{}/oauth/jwks", mock_server_url))
            .build()?;

        let esi_client = eve_esi::Client::builder()
            .config(esi_config)
            .user_agent(TEST_USER_AGENT)
            .client_id(TEST_ESI_CLIENT_ID)
            .client_secret(TEST_ESI_CLIENT_SECRET)
            .callback_url(TEST_CALLBACK_URL)
            .build()?;

        let store = Arc::new(MemoryStore::default());
        let session = Session::new(None, store, None);

        let db = Database::connect("sqlite::memory:").await.unwrap();

        Ok(TestSetup {
            server: mock_server,
            state: AppState {
                db,
                esi_client: esi_client,
            },
            session,
        })
    }
}
