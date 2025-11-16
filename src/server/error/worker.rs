use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::server::error::InternalServerError;

#[derive(Error, Debug)]
pub enum WorkerError {
    #[error("Invalid job identity string: {0}")]
    InvalidJobIdentity(String),
    #[error("Affiliation job batch size {size} exceeds maximum of {max}")]
    AffiliationBatchTooLarge { size: usize, max: usize },
}

impl IntoResponse for WorkerError {
    fn into_response(self) -> Response {
        InternalServerError(self).into_response()
    }
}
