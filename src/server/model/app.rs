//! Application state models.
//!
//! This module defines the central application state structure that is shared across all
//! HTTP handlers via Axum's state management. The `AppState` contains core dependencies
//! like the database connection, ESI client, and worker system that handlers need to
//! process requests and dispatch background jobs.

use sea_orm::DatabaseConnection;

use crate::server::worker::Worker;

/// Central application state shared across HTTP handlers.
///
/// `AppState` aggregates the core dependencies needed by HTTP controllers and services,
/// including database access, ESI client for EVE Online API calls, and the worker system
/// for background job processing. This struct is cloned for each request handler (all fields
/// use Arc internally for cheap cloning) and injected via Axum's `State` extractor.
///
/// # Fields
/// - `db` - Database connection pool for querying and persisting data
/// - `esi_client` - EVE Online ESI API client for fetching game data
/// - `worker` - Worker system for dispatching and managing background jobs
///
/// # Example
/// ```ignore
/// pub async fn handler(
///     State(state): State<AppState>,
/// ) -> Result<impl IntoResponse, Error> {
///     // Use state.db for database queries
///     // Use state.esi_client for ESI API calls
///     // Use state.worker for background jobs
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool for data persistence and queries.
    pub db: DatabaseConnection,

    /// EVE Online ESI client for fetching game data and authentication.
    pub esi_client: eve_esi::Client,

    /// Worker system for dispatching background jobs to the Redis-backed queue.
    pub worker: Worker,
}
