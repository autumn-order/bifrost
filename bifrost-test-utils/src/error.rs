//! Error types for test utilities.
//!
//! This module defines the error type returned by test utility operations,
//! wrapping underlying errors from ESI client, database, session, and Redis operations.

use thiserror::Error;

/// Error type for test utility operations.
///
/// Wraps various error types that can occur during test setup and execution,
/// including ESI client errors, database errors, session errors, and Redis errors.
#[derive(Error, Debug)]
pub enum TestError {
    /// Error from EVE ESI client operations
    ///
    /// Occurs when ESI client initialization fails or ESI API calls fail during test setup.
    #[error(transparent)]
    EsiError(#[from] eve_esi::Error),

    /// Error from database operations
    ///
    /// Occurs when database connection, table creation, or query execution fails.
    #[error(transparent)]
    DbErr(#[from] sea_orm::DbErr),

    /// Error from session store operations
    ///
    /// Occurs when session initialization or session store operations fail.
    #[error(transparent)]
    SessionError(#[from] tower_sessions::session::Error),

    /// Error from Redis operations
    ///
    /// Occurs when Redis client operations fail during test setup.
    #[error(transparent)]
    FredError(#[from] fred::error::Error),
}
