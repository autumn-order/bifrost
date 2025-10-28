use std::sync::Arc;

use mockito::{Mock, Server, ServerGuard};
use sea_orm::Database;
use tower_sessions::{MemoryStore, Session};

use crate::server::{
    data::{
        eve::{character::CharacterRepository, corporation::CorporationRepository},
        user::{user_character::UserCharacterRepository, UserRepository},
    },
    error::Error,
    model::app::AppState,
    util::test::{
        eve::mock::{mock_character, mock_corporation},
        mockito::{character::mock_character_endpoint, corporation::mock_corporation_endpoint},
    },
};

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

    let db = Database::connect("sqlite::memory:").await.unwrap();

    let state = AppState {
        db,
        esi_client: esi_client,
    };

    TestSetup {
        server: mock_server,
        state,
        session,
    }
}

/// Inserts mock data for an EVE Online corporation
pub async fn test_setup_create_corporation(
    test: &TestSetup,
    corporation_id: i64,
) -> Result<entity::eve_corporation::Model, Error> {
    let corporation_repo = CorporationRepository::new(&test.state.db);

    let faction_id = None;
    let alliance_id = None;
    let mock_corporation = mock_corporation(alliance_id, faction_id);

    let corporation = corporation_repo
        .create(corporation_id, mock_corporation, None, None)
        .await?;

    Ok(corporation)
}

/// Inserts mock data for an EVE Online character
pub async fn test_setup_create_character(
    test: &TestSetup,
    character_id: i64,
    corporation: entity::eve_corporation::Model,
) -> Result<entity::eve_character::Model, Error> {
    let character_repo = CharacterRepository::new(&test.state.db);

    let faction_id = None;
    let alliance_id = None;
    let mock_character = mock_character(corporation.corporation_id, alliance_id, faction_id);

    let character = character_repo
        .create(character_id, mock_character, corporation.id, None)
        .await?;

    Ok(character)
}

/// Inserts mock data for a user which owns a character
pub async fn test_setup_create_user_with_character(
    test: &TestSetup,
    character: entity::eve_character::Model,
) -> Result<entity::bifrost_user_character::Model, Error> {
    let user_repo = UserRepository::new(&test.state.db);
    let user_character_repo = UserCharacterRepository::new(&test.state.db);

    let user = user_repo.create(character.id).await?;
    let character_ownership = user_character_repo
        .create(user.id, character.id, "test_owner_hash".to_string())
        .await?;

    Ok(character_ownership)
}

/// Provides mock ESI endpoints required for creating a new character
pub async fn test_setup_create_character_endpoints(test: &mut TestSetup) -> (Mock, Mock) {
    let faction_id = None;
    let alliance_id = None;
    let corporation_id = 1;
    let mock_corporation = mock_corporation(alliance_id, faction_id);

    let mock_character = mock_character(corporation_id, alliance_id, faction_id);

    let mock_corporation_endpoint =
        mock_corporation_endpoint(&mut test.server, "/corporations/1", mock_corporation, 1);
    let mock_character_endpoint =
        mock_character_endpoint(&mut test.server, "/characters/1", mock_character, 1);

    (mock_character_endpoint, mock_corporation_endpoint)
}
