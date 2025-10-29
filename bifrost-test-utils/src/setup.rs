use std::sync::Arc;

use mockito::{Server, ServerGuard};
use sea_orm::{sea_query::TableCreateStatement, ConnectionTrait, Database, DatabaseConnection};
use tower_sessions::{MemoryStore, Session};

use crate::{
    constant::{TEST_CALLBACK_URL, TEST_ESI_CLIENT_ID, TEST_ESI_CLIENT_SECRET, TEST_USER_AGENT},
    error::TestError,
};

pub struct AppState {
    pub db: DatabaseConnection,
    pub esi_client: eve_esi::Client,
}

pub struct TestSetup {
    pub server: ServerGuard,
    pub state: AppState,
    pub session: Session,
}

impl TestSetup {
    pub async fn new() -> Result<Self, TestError> {
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

    pub async fn with_tables(&self, stmts: Vec<TableCreateStatement>) -> Result<(), TestError> {
        for stmt in stmts {
            self.state.db.execute(&stmt).await?;
        }

        Ok(())
    }
}

#[macro_export]
macro_rules! test_setup {
    // Pattern 1: No entities provided
    () => {{
        TestSetup::new().await
    }};

    // Pattern 2: Entities provided
    ($($entity:expr),+ $(,)?) => {{
        async {
            let setup = TestSetup::new().await?;

            let schema = sea_orm::Schema::new(sea_orm::DbBackend::Sqlite);
            let stmts = vec![
                $(schema.create_table_from_entity($entity),)+
            ];
            setup.with_tables(stmts).await?;

            Ok::<_, $crate::error::TestError>(setup)
        }.await
    }};
}
