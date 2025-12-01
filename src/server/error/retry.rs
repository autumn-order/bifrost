use sea_orm::SqlErr;

use super::Error;

/// Strategy for handling errors in a retry context
pub enum ErrorRetryStrategy {
    /// Retry with exponential backoff (server errors)
    Retry,
    /// Failed permanently (bad request)
    Fail,
}

impl Error {
    /// Determine error retry strategy based upon application Error type
    pub fn to_retry_strategy(&self) -> ErrorRetryStrategy {
        match self {
            // ESI request errors - internal errors, bad requests, rate limiting
            Error::EsiError(eve_esi::Error::ReqwestError(reqwest_error)) => {
                if let Some(status) = reqwest_error.status() {
                    match status {
                        // 500 - Internal Server Error
                        //
                        // ESI is temporarily unavailable, backoff and retry later, if ESI internal
                        // errors accumulate trigger global circuit breaker and defer all ESI requests
                        // until ping succeeds to avoid hammering ESI.
                        s if s.is_server_error() => ErrorRetryStrategy::Retry,

                        // 400 - Client Error
                        // We're making invalid requests to ESI, this is a flaw in the code that needs
                        // to be fixed.
                        s if s.is_client_error() => ErrorRetryStrategy::Fail,

                        // Unexpected response
                        _ => ErrorRetryStrategy::Fail,
                    }
                } else {
                    // Network error or connection issue - should retry
                    ErrorRetryStrategy::Retry
                }
            }

            Self::DbErr(db_err) => {
                if let Some(sql_err) = db_err.sql_err() {
                    match sql_err {
                        // Foreign key constraint violations are permanent failures
                        SqlErr::ForeignKeyConstraintViolation(_) => ErrorRetryStrategy::Fail,
                        // Unique constraint violations are also permanent
                        SqlErr::UniqueConstraintViolation(_) => ErrorRetryStrategy::Fail,
                        // Other SQL errors should retry
                        _ => ErrorRetryStrategy::Retry,
                    }
                } else {
                    // Other database errors (connection issues, etc.) should retry
                    ErrorRetryStrategy::Retry
                }
            }

            // ESI errors - other errors, OAuth, parsing, etc
            Self::EsiError(_) => ErrorRetryStrategy::Fail,

            // Configuration errors - permanent failures, won't resolve with retry
            Self::ConfigError(_) => ErrorRetryStrategy::Fail,

            // Auth errors - permanent failures (bad requests, missing data)
            Self::AuthError(_) => ErrorRetryStrategy::Fail,

            // Parse errors - permanent failures (bad data format)
            Self::ParseError(_) => ErrorRetryStrategy::Fail,

            // InternalError - permanent failures (internal error within Bifrost's code)
            Self::InternalError(_) => ErrorRetryStrategy::Fail,

            // Worker errors - permanent failures (validation errors)
            Self::WorkerError(_) => ErrorRetryStrategy::Fail,

            // Internal EVE-related errors - might resolve after cache update, but rare
            // Treating as permanent since retrying won't help in the short term
            Self::EveError(_) => ErrorRetryStrategy::Fail,

            // Session errors - transient, could be Redis connection issues
            Self::SessionError(_) => ErrorRetryStrategy::Retry,
            Self::SessionRedisError(_) => ErrorRetryStrategy::Retry,
        }
    }
}
