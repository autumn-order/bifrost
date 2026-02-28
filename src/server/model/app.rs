//! Application state models.
//!
//! This module defines the central application state structure that is shared across all
//! HTTP handlers via Axum's state management. The `AppState` contains core dependencies
//! like the database connection, ESI client, and worker system that handlers need to
//! process requests and dispatch background jobs.

use sea_orm::DatabaseConnection;

use crate::server::{service::eve::esi::EsiProvider, worker::Worker};

/// Central application state shared across HTTP handlers.
///
/// `AppState` aggregates the core dependencies needed by HTTP controllers and services,
/// including database access, ESI provider for EVE Online API calls with circuit breaker
/// protection, and the worker system for background job processing. This struct is cloned
/// for each request handler (all fields use Arc internally for cheap cloning) and injected
/// via Axum's `State` extractor.
///
/// # Fields
/// - `db` - Database connection pool for querying and persisting data
/// - `esi_provider` - ESI provider with circuit breaker protection for EVE Online API calls
/// - `worker` - Worker system for dispatching and managing background jobs
///
/// # Example
/// ```ignore
/// pub async fn handler(
///     State(state): State<AppState>,
/// ) -> Result<impl IntoResponse, AppError> {
///     // Use state.db for database queries
///     // Use state.esi_provider for ESI API calls (with circuit breaker)
///     // Use state.esi_provider.oauth2() for OAuth2 flows
///     // Use state.worker for background jobs
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool for data persistence and queries.
    pub db: DatabaseConnection,

    /// ESI provider with circuit breaker protection for EVE Online API calls.
    /// Use `.client()` method to access the underlying client for operations like OAuth2.
    pub esi_provider: EsiProvider,

    /// Worker system for dispatching background jobs to the Redis-backed queue.
    pub worker: Worker,
}
