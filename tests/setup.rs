use std::sync::Arc;

use bifrost::server::model::app::AppState;
use tower_sessions::{MemoryStore, Session};

pub static USER_AGENT: &str =
    "MyApp/1.0 (contact@example.com; +https://github.com/autumn-order/bifrost)";
static ESI_CLIENT_ID: &str = "esi_client_id";
static ESI_CLIENT_SECRET: &str = "esi_client_secret";
static CALLBACK_URL: &str = "http://localhost:8080/auth/callback";

// Returns a tuple with [`AppState`] & [`Session`] used across integration tests
pub fn setup() -> (AppState, Session) {
    let esi_client = eve_esi::Client::builder()
        .user_agent(USER_AGENT)
        .client_id(ESI_CLIENT_ID)
        .client_secret(ESI_CLIENT_SECRET)
        .callback_url(CALLBACK_URL)
        .build()
        .expect("Failed to build ESI client");

    let store = Arc::new(MemoryStore::default());
    let session = Session::new(None, store, None);

    let state = AppState {
        esi_client: esi_client,
    };

    (state, session)
}
