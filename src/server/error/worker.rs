//! Worker queue error types.
//!
//! This module defines errors related to worker job validation, serialization, and scheduling.
//! Worker errors typically indicate programming bugs (invalid job parameters) or Redis/queue
//! infrastructure issues that prevent jobs from being properly enqueued or processed.

use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::server::error::InternalServerError;

/// Worker queue error type.
///
/// These errors occur during worker job creation, validation, serialization, or scheduling.
/// All worker errors are treated as internal server errors (500) since they indicate issues
/// with the background job system rather than client errors.
#[derive(Error, Debug)]
pub enum WorkerError {
    /// Affiliation job batch size exceeds ESI's request limit.
    ///
    /// This error occurs when attempting to create an affiliation update job with more
    /// character IDs than ESI's bulk affiliation endpoint allows (typically 1000). This
    /// indicates a programming error in the batch creation logic.
    ///
    /// # Fields
    /// - `size` - The attempted batch size
    /// - `max` - The maximum allowed batch size (ESI_AFFILIATION_REQUEST_LIMIT)
    #[error("Affiliation job batch size {size} exceeds maximum of {max}")]
    AffiliationBatchTooLarge { size: usize, max: usize },

    /// Failed to serialize or deserialize a WorkerJob.
    ///
    /// This error occurs when converting a WorkerJob to/from JSON for Redis storage.
    /// It may indicate a schema mismatch or corruption in the Redis data, or an issue
    /// with the serde implementation.
    #[error("Failed to serialize/deserialize WorkerJob: {0}")]
    SerializationError(String),

    /// Failed to schedule a task in the worker queue.
    ///
    /// This error occurs when the worker queue system cannot accept a new job, typically
    /// due to Redis connection issues, queue full conditions, or Lua script execution
    /// failures.
    #[error("Failed to schedule task: {0}")]
    Scheduler(String),
}

/// Converts worker errors into HTTP responses.
///
/// All worker errors are treated as internal server errors (500) since they indicate
/// issues with the background job system rather than client errors. The error is logged
/// for debugging and a generic error message is returned to the client.
///
/// # Returns
/// A 500 Internal Server Error response with a generic error message
impl IntoResponse for WorkerError {
    fn into_response(self) -> Response {
        InternalServerError(self).into_response()
    }
}
