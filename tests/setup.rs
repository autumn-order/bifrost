use std::sync::Arc;

use bifrost::server::model::app::AppState;
use mockito::{Server, ServerGuard};
use tower_sessions::{MemoryStore, Session};

pub static TEST_USER_AGENT: &str =
    "MyApp/1.0 (contact@example.com; +https://github.com/autumn-order/bifrost)";
static TEST_ESI_CLIENT_ID: &str = "esi_client_id";
static TEST_ESI_CLIENT_SECRET: &str = "esi_client_secret";
static TEST_CALLBACK_URL: &str = "http://localhost:8080/auth/callback";

pub struct TestSetup {
    pub server: ServerGuard,
    pub state: AppState,
    pub session: Session,
}

// Returns a tuple with [`AppState`] & [`Session`] used across integration tests
pub async fn test_setup() -> TestSetup {
    let mock_server = Server::new_async().await;
    let mock_server_url = mock_server.url();

    let esi_config = eve_esi::Config::builder()
        .esi_url(&mock_server_url)
        .token_url(&format!("{}/v2/oauth/token", mock_server.url()))
        .jwk_url(&format!("{}/oauth/jwks", mock_server_url))
        .build()
        .expect("Failed to build ESI client config");

    let esi_client = eve_esi::Client::builder()
        .config(esi_config)
        .user_agent(TEST_USER_AGENT)
        .client_id(TEST_ESI_CLIENT_ID)
        .client_secret(TEST_ESI_CLIENT_SECRET)
        .callback_url(TEST_CALLBACK_URL)
        .build()
        .expect("Failed to build ESI client");

    let store = Arc::new(MemoryStore::default());
    let session = Session::new(None, store, None);

    let state = AppState {
        esi_client: esi_client,
    };

    TestSetup {
        server: mock_server,
        state,
        session,
    }
}
