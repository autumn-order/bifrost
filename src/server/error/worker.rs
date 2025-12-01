use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::server::error::InternalServerError;

#[derive(Error, Debug)]
pub enum WorkerError {
    #[error("Affiliation job batch size {size} exceeds maximum of {max}")]
    AffiliationBatchTooLarge { size: usize, max: usize },
    #[error("Failed to serialize/deserialize WorkerJob: {0}")]
    SerializationError(String),
    #[error("Failed to schedule task: {0}")]
    Scheduler(String),
}

impl IntoResponse for WorkerError {
    fn into_response(self) -> Response {
        InternalServerError(self).into_response()
    }
}
