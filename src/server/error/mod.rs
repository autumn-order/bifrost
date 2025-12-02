//! Error types for the Bifrost server application.
//!
//! This module provides a comprehensive error handling system with specialized error types
//! for different domains (authentication, configuration, EVE Online integration, worker queue).
//! All errors implement `IntoResponse` for Axum HTTP responses and use `thiserror` for
//! ergonomic error definitions with automatic `Display` and `Error` trait implementations.

pub mod auth;
pub mod config;
pub mod eve;
pub mod retry;
pub mod worker;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use dioxus_logger::tracing;
use thiserror::Error;

use crate::{
    model::api::ErrorDto,
    server::error::{auth::AuthError, config::ConfigError, eve::EveError, worker::WorkerError},
};

/// Main error type for the Bifrost server application.
///
/// This enum aggregates all domain-specific error types and external library errors into a
/// single unified error type. It uses `thiserror`'s `#[from]` attribute to enable automatic
/// conversion from underlying error types via the `?` operator. The `IntoResponse` implementation
/// maps errors to appropriate HTTP responses for API consumers.
///
/// # Error Categories
/// - Configuration errors (missing/invalid environment variables)
/// - Authentication errors (session, CSRF, user validation)
/// - EVE Online errors (ESI interactions, faction lookup)
/// - Worker queue errors (job validation, scheduling)
/// - External library errors (database, ESI client, sessions, scheduler)
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration error (missing or invalid environment variables).
    #[error(transparent)]
    ConfigError(#[from] ConfigError),
    /// Authentication error (session, CSRF, user/character validation).
    #[error(transparent)]
    AuthError(#[from] AuthError),
    /// EVE Online-specific error (faction lookup, ESI data issues).
    #[error(transparent)]
    EveError(#[from] EveError),
    /// Worker queue error (job validation, serialization, scheduling).
    #[error(transparent)]
    WorkerError(#[from] WorkerError),
    /// Parse error (failed to parse a value from string or other format).
    #[error("Failed to parse value: {0:?}")]
    ParseError(String),
    /// Internal error indicating a bug in Bifrost's code.
    ///
    /// This error should never occur in normal operation and indicates a programming error
    /// that needs to be reported as a GitHub issue.
    #[error("Internal error with Bifrost's code, please open a GitHub issue as this indicates a bug: {0:?}")]
    InternalError(String),
    /// ESI client error (API requests, OAuth, rate limiting).
    #[error(transparent)]
    EsiError(#[from] eve_esi::Error),
    /// Database error (query failures, connection issues, constraint violations).
    #[error(transparent)]
    DbErr(#[from] sea_orm::DbErr),
    /// Session error (session retrieval, storage, serialization).
    #[error(transparent)]
    SessionError(#[from] tower_sessions::session::Error),
    /// Redis session store error (connection, command execution).
    #[error(transparent)]
    SessionRedisError(#[from] tower_sessions_redis_store::fred::prelude::Error),
    /// Cron scheduler error (job registration, scheduler startup).
    #[error(transparent)]
    SchedulerError(#[from] tokio_cron_scheduler::JobSchedulerError),
}

/// Converts application errors into HTTP responses.
///
/// Maps domain-specific errors to appropriate HTTP status codes and JSON error responses.
/// Most errors are treated as internal server errors (500) with logging, while specific
/// error types like `AuthError` and `EveError` have custom response mappings.
///
/// # Returns
/// - 400 Bad Request - For authentication failures (CSRF, invalid character selection)
/// - 404 Not Found - For missing users or resources
/// - 500 Internal Server Error - For all other errors (with error logging)
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::ConfigError(err) => err.into_response(),
            Self::AuthError(err) => err.into_response(),
            Self::EveError(err) => err.into_response(),
            err => InternalServerError(err).into_response(),
        }
    }
}

/// Wrapper type for converting any displayable error into a 500 Internal Server Error response.
///
/// This struct logs the error message and returns a generic "Internal server error" message
/// to the client to avoid leaking implementation details. Used as a fallback for errors that
/// don't have specific HTTP response mappings.
pub struct InternalServerError<E>(pub E);

/// Converts wrapped errors into 500 Internal Server Error responses.
///
/// Logs the full error message for debugging, but returns a generic error message to the
/// client to avoid exposing internal implementation details or sensitive information.
///
/// # Arguments
/// - `E` - Any type that implements `Display` (typically an error type)
///
/// # Returns
/// A 500 Internal Server Error response with a generic error message JSON body
impl<E: std::fmt::Display> IntoResponse for InternalServerError<E> {
    fn into_response(self) -> Response {
        tracing::error!("{}", self.0);

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorDto {
                error: "Internal server error".to_string(),
            }),
        )
            .into_response()
    }
}
