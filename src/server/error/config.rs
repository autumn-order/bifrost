use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::server::error::InternalServerError;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),
    #[error("Invalid value for environment variable {var}: {reason}")]
    InvalidEnvValue { var: String, reason: String },
}

impl IntoResponse for ConfigError {
    fn into_response(self) -> Response {
        InternalServerError(self).into_response()
    }
}
