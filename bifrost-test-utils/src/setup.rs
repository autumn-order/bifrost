use std::sync::Arc;

use mockito::{Mock, Server, ServerGuard};
use sea_orm::{sea_query::TableCreateStatement, ConnectionTrait, Database, DatabaseConnection};
use tower_sessions::{MemoryStore, Session};

use crate::{
    constant::{TEST_CALLBACK_URL, TEST_ESI_CLIENT_ID, TEST_ESI_CLIENT_SECRET, TEST_USER_AGENT},
    error::TestError,
};

pub struct TestAppState {
    pub db: DatabaseConnection,
    pub esi_client: eve_esi::Client,
}

pub struct TestSetup {
    pub server: ServerGuard,
    pub state: TestAppState,
    pub session: Session,
    pub mocks: Vec<Mock>,
}

impl TestSetup {
    /// Convert TestAppState into any type that can be constructed from its fields.
    /// This allows conversion to AppState without creating a circular dependency.
    ///
    /// # Example
    /// ```
    /// let app_state: AppState = test_app_state.into_app_state();
    /// ```
    pub fn state<T>(&self) -> T
    where
        T: From<(DatabaseConnection, eve_esi::Client)>,
    {
        T::from((self.state.db.clone(), self.state.esi_client.clone()))
    }
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
            state: TestAppState { db, esi_client },
            session,
            mocks: Vec::new(),
        })
    }

    pub async fn with_tables(&self, stmts: Vec<TableCreateStatement>) -> Result<(), TestError> {
        for stmt in stmts {
            self.state.db.execute(&stmt).await?;
        }

        Ok(())
    }

    /// Assert all mock endpoints were called as expected.
    ///
    /// Calls `assert()` on all mocks created by the TestBuilder to verify
    /// they were invoked the expected number of times.
    ///
    /// # Panics
    /// Panics if any mock endpoint was not called the expected number of times
    pub fn assert_mocks(&self) {
        for mock in &self.mocks {
            mock.assert();
        }
    }
}

#[macro_export]
macro_rules! test_setup_with_tables {
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

#[macro_export]
macro_rules! test_setup_with_user_tables {
    // Pattern 1: No entities provided
    () => {{
        async {
            let setup = TestSetup::new().await?;

            let schema = sea_orm::Schema::new(sea_orm::DbBackend::Sqlite);
            let stmts = vec![
                schema.create_table_from_entity(entity::prelude::EveFaction),
                schema.create_table_from_entity(entity::prelude::EveAlliance),
                schema.create_table_from_entity(entity::prelude::EveCorporation),
                schema.create_table_from_entity(entity::prelude::EveCharacter),
                schema.create_table_from_entity(entity::prelude::BifrostUser),
                schema.create_table_from_entity(entity::prelude::BifrostUserCharacter)
            ];
            setup.with_tables(stmts).await?;

            Ok::<_, $crate::error::TestError>(setup)
        }.await
    }};

    // Pattern 2: Entities provided
    ($($entity:expr),+ $(,)?) => {{
        async {
            let setup = TestSetup::new().await?;

            let schema = sea_orm::Schema::new(sea_orm::DbBackend::Sqlite);
            let stmts = vec![
                schema.create_table_from_entity(entity::prelude::EveFaction),
                schema.create_table_from_entity(entity::prelude::EveAlliance),
                schema.create_table_from_entity(entity::prelude::EveCorporation),
                schema.create_table_from_entity(entity::prelude::EveCharacter),
                schema.create_table_from_entity(entity::prelude::BifrostUser),
                schema.create_table_from_entity(entity::prelude::BifrostUserCharacter),
                $(schema.create_table_from_entity($entity),)+
            ];
            setup.with_tables(stmts).await?;

            Ok::<_, $crate::error::TestError>(setup)
        }.await
    }};
}
