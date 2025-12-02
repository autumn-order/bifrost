//! Configuration error types.
//!
//! This module defines errors related to application configuration, particularly environment
//! variable validation. Configuration errors are typically encountered during application
//! startup when required environment variables are missing or contain invalid values.

use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::server::error::InternalServerError;

/// Configuration error type for environment variable validation failures.
///
/// These errors occur during application startup when the configuration system detects
/// missing or invalid environment variables. Configuration errors are always treated as
/// fatal and result in 500 Internal Server Error responses if encountered during request
/// handling, though typically they prevent the application from starting at all.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Required environment variable is not set.
    ///
    /// The application requires this environment variable to be defined. Check the
    /// documentation or `.env.example` file for required configuration variables.
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),

    /// Environment variable value is invalid or malformed.
    ///
    /// The environment variable is set but contains a value that cannot be parsed or
    /// is not within acceptable bounds. The `reason` field provides details about why
    /// the value was rejected.
    ///
    /// # Fields
    /// - `var` - Name of the environment variable with invalid value
    /// - `reason` - Explanation of why the value is invalid
    #[error("Invalid value for environment variable {var}: {reason}")]
    InvalidEnvValue { var: String, reason: String },
}

/// Converts configuration errors into HTTP responses.
///
/// All configuration errors are treated as internal server errors (500) since they
/// indicate a deployment or setup issue rather than a client error. The error is
/// logged for debugging and a generic error message is returned to the client.
///
/// # Returns
/// A 500 Internal Server Error response with a generic error message
impl IntoResponse for ConfigError {
    fn into_response(self) -> Response {
        InternalServerError(self).into_response()
    }
}
