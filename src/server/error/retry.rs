//! Error retry strategy determination.
//!
//! This module defines retry strategies for different error types, allowing the retry system
//! to distinguish between transient errors (that should be retried with exponential backoff)
//! and permanent errors (that should fail immediately). This is crucial for worker jobs and
//! service operations that interact with external systems like ESI and Redis.

use sea_orm::DbErr;

use super::Error;

/// Strategy for handling errors in a retry context.
///
/// Determines whether an operation should be retried with exponential backoff or should
/// fail permanently. This enum is used by the retry system to make intelligent decisions
/// about error recovery based on the error type.
pub enum ErrorRetryStrategy {
    /// Retry the operation with exponential backoff.
    ///
    /// Used for transient errors that may resolve themselves, such as:
    /// - ESI server errors (500-level responses)
    /// - Network/connection issues
    /// - Database connection acquisition failures
    /// - Redis connection errors
    Retry,

    /// Fail permanently without retry.
    ///
    /// Used for errors that won't resolve with retry, such as:
    /// - Client errors (400-level responses indicating bad requests)
    /// - Configuration errors
    /// - Parse errors
    /// - Data constraint violations
    /// - Programming bugs (internal errors)
    Fail,
}

impl Error {
    /// Determines the appropriate retry strategy for this error.
    ///
    /// Analyzes the error type and context to decide whether the operation should be retried
    /// with exponential backoff or should fail immediately. This method categorizes errors as
    /// either transient (likely to resolve with retry) or permanent (indicating a bug or
    /// configuration issue).
    ///
    /// # Retry Strategy Guidelines
    ///
    /// **Transient errors (Retry):**
    /// - ESI 500-level server errors - ESI is temporarily unavailable
    /// - Network/connection errors - May resolve with retry
    /// - Database connection acquisition/connection errors - Connection pool may recover
    /// - Session/Redis errors - Connection issues that may be temporary
    ///
    /// **Permanent errors (Fail):**
    /// - ESI 400-level client errors - Invalid request (programming bug)
    /// - Database query errors - Constraint violations, bad queries
    /// - Configuration errors - Missing/invalid environment variables
    /// - Parse errors - Malformed data that won't change
    /// - Internal errors - Bugs in Bifrost's code
    ///
    /// # Returns
    /// - `ErrorRetryStrategy::Retry` - Operation should be retried with exponential backoff
    /// - `ErrorRetryStrategy::Fail` - Operation should fail permanently without retry
    pub fn to_retry_strategy(&self) -> ErrorRetryStrategy {
        match self {
            // ESI request errors - categorize by HTTP status code
            Error::EsiError(eve_esi::Error::ReqwestError(reqwest_error)) => {
                if let Some(status) = reqwest_error.status() {
                    match status {
                        // 5xx Server Errors - ESI is temporarily unavailable
                        //
                        // Retry with backoff. If ESI internal errors accumulate, a global
                        // circuit breaker should be triggered to defer all ESI requests until
                        // a health check succeeds, avoiding hammering an already-failing ESI.
                        s if s.is_server_error() => ErrorRetryStrategy::Retry,

                        // 4xx Client Errors - Invalid request (programming bug)
                        //
                        // We're making invalid requests to ESI. This indicates a flaw in the
                        // code that needs to be fixed. Retrying won't help.
                        s if s.is_client_error() => ErrorRetryStrategy::Fail,

                        // Unexpected HTTP status code - treat as permanent failure
                        _ => ErrorRetryStrategy::Fail,
                    }
                } else {
                    // Network error or connection issue without HTTP status - likely transient
                    ErrorRetryStrategy::Retry
                }
            }

            Self::DbErr(db_err) => {
                match db_err {
                    // Connection acquisition failures - transient, connection pool may recover
                    DbErr::ConnectionAcquire(_) => ErrorRetryStrategy::Retry,

                    // Connection errors - transient, database server may recover
                    DbErr::Conn(_) => ErrorRetryStrategy::Retry,

                    // All other database errors are permanent failures:
                    // - Query errors (syntax, constraint violations)
                    // - Type conversion errors (schema mismatch)
                    // - Migration errors (schema issues)
                    // - Record not found/inserted/updated (data integrity issues)
                    //
                    // These indicate programming bugs or data issues that won't resolve with retry.
                    _ => ErrorRetryStrategy::Fail,
                }
            }

            // Session errors - transient, typically Redis connection issues
            Self::SessionError(_) => ErrorRetryStrategy::Retry,

            // Redis session store errors - transient connection/command failures
            Self::SessionRedisError(_) => ErrorRetryStrategy::Retry,

            // Other ESI errors - permanent failures (OAuth, parsing, etc.)
            Self::EsiError(_) => ErrorRetryStrategy::Fail,

            // Configuration errors - permanent failures (missing/invalid env vars)
            Self::ConfigError(_) => ErrorRetryStrategy::Fail,

            // Auth errors - permanent failures (CSRF, bad credentials, missing data)
            Self::AuthError(_) => ErrorRetryStrategy::Fail,

            // Parse errors - permanent failures (malformed data that won't change)
            Self::ParseError(_) => ErrorRetryStrategy::Fail,

            // Internal errors - permanent failures (bugs in Bifrost's code)
            Self::InternalError(_) => ErrorRetryStrategy::Fail,

            // Worker errors - permanent failures (job validation, serialization)
            Self::WorkerError(_) => ErrorRetryStrategy::Fail,

            // Job scheduler errors - permanent failures (invalid cron, config issues)
            Self::SchedulerError(_) => ErrorRetryStrategy::Fail,

            // EVE-related errors - permanent failures (might resolve after ESI cache update, but rare)
            Self::EveError(_) => ErrorRetryStrategy::Fail,
        }
    }
}
