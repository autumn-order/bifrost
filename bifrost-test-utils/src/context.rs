//! Test context structure and utilities.
//!
//! This module provides the `TestContext` returned by `TestBuilder` for Phase 2 test execution.
//! The context includes an in-memory SQLite database, mock ESI server, configured ESI client,
//! and session store for testing authentication flows.

use std::sync::Arc;

use mockito::{Mock, Server, ServerGuard};
use sea_orm::{sea_query::TableCreateStatement, ConnectionTrait, Database, DatabaseConnection};
use tower_sessions::{MemoryStore, Session};

use crate::{
    constant::{TEST_CALLBACK_URL, TEST_ESI_CLIENT_ID, TEST_ESI_CLIENT_SECRET, TEST_USER_AGENT},
    error::TestError,
};

/// Test context structure returned by `TestBuilder`
///
/// This struct is the result of calling `TestBuilder::build()` and provides
/// access to the test environment including:
/// - Mock ESI server
/// - Database connection
/// - ESI client configured to use the mock server
/// - Session store
/// - Collection of mock endpoints for assertion
///
/// # Usage
///
/// Most users should create this via [`TestBuilder`](crate::TestBuilder) rather
/// than constructing it directly.
///
/// ```ignore
/// let test = TestBuilder::new().build().await?;
///
/// // Access the database
/// let db = &test.db;
///
/// // Access the ESI client
/// let client = &test.esi_client;
///
/// // Access fixtures helpers
/// test.eve().insert_mock_faction(1).await?;
/// test.user().insert_user_with_mock_character(1, 1, None, None).await?;
///
/// // Assert all mocks were called
/// test.assert_mocks();
/// ```
pub struct TestContext {
    /// Database connection to in-memory SQLite database
    pub db: DatabaseConnection,
    /// ESI client configured to use mock server
    pub esi_client: eve_esi::Client,
    /// Session store for test authentication flows
    pub session: Session,

    /// Mock HTTP server for ESI endpoints
    pub(crate) server: ServerGuard,
    /// Collection of mock HTTP endpoints for assertion
    pub(crate) mocks: Vec<Mock>,
}

impl TestContext {
    /// Convert database and ESI client into any type that can be constructed from them
    ///
    /// This allows conversion to AppState without creating a circular dependency
    /// between the test-utils crate and the main bifrost crate.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // In integration tests
    /// let app_state: AppState = test.to_app_state();
    /// ```
    pub fn to_app_state<T>(&self) -> T
    where
        T: From<(DatabaseConnection, eve_esi::Client)>,
    {
        T::from((self.db.clone(), self.esi_client.clone()))
    }
}

impl TestContext {
    /// Create a new test context.
    ///
    /// Initializes a complete test environment with an in-memory SQLite database,
    /// mock ESI server, ESI client configured to use the mock server, and session store.
    ///
    /// # Returns
    /// - `Ok(TestContext)` - Fully initialized test context
    /// - `Err(TestError::EsiError)` - ESI client or config builder failed
    /// - `Err(TestError::DbErr)` - Database connection failed
    pub(crate) async fn new() -> Result<Self, TestError> {
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

        Ok(TestContext {
            server: mock_server,
            db,
            esi_client,
            session,
            mocks: Vec::new(),
        })
    }

    /// Create database tables from schema statements.
    ///
    /// Executes CREATE TABLE statements for all provided table schemas. Used internally
    /// by TestBuilder to set up the database schema during test initialization.
    ///
    /// # Arguments
    /// - `stmts` - Vector of CREATE TABLE statements to execute
    ///
    /// # Returns
    /// - `Ok(())` - All tables created successfully
    /// - `Err(TestError::DbErr)` - Table creation failed
    pub(crate) async fn with_tables(
        &self,
        stmts: Vec<TableCreateStatement>,
    ) -> Result<(), TestError> {
        for stmt in stmts {
            self.db.execute(&stmt).await?;
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
