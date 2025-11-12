use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use dioxus_logger::tracing::{debug, error};
use thiserror::Error;

use crate::model::api::ErrorDto;

#[derive(Error, Debug)]
pub enum Error {
    // This is an edge case error, it'll probably end up resolving itself after 24 hours should it occur
    // once the EVE Online ESI faction cache updates.
    //
    // This issue is not fatal, the affected characters/corporations/alliances
    // can safely be set to not be a member of this faction as a temporary solution
    // until the issue is resolved.
    #[error(
        "Failed to find information for EVE Online NPC faction ID: {0:?}\n\
        \n\
        This should never occur but if it has please open a GitHub issue so we can look into it:\n\
        https://github.com/autumn-order/bifrost\n\
        \n\
        For now, this faction will not be saved to the database and no characters, corporations, or alliances
        will show membership to this faction until the issue is resolved."
    )]
    EveFactionNotFound(i64),
    #[error("Failed to parse value: {0:?}")]
    ParseError(String),
    #[error("Failed to login user due to CSRF state present in session store but without a value")]
    AuthCsrfEmptySession,
    #[error("Failed to login user due to CSRF state mismatch")]
    AuthCsrfInvalidState,
    #[error("User ID is not present in session")]
    AuthUserNotInSession,
    #[error("User ID {0:?} not found in database despite having an active session")]
    AuthUserNotInDatabase(i32),
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),
    #[error("Invalid value for environment variable {var}: {reason}")]
    InvalidEnvValue { var: String, reason: String },
    #[error(transparent)]
    EsiError(#[from] eve_esi::Error),
    #[error(transparent)]
    DbErr(#[from] sea_orm::DbErr),
    #[error(transparent)]
    SessionError(#[from] tower_sessions::session::Error),
    #[error(transparent)]
    SessionRedisError(#[from] tower_sessions_redis_store::fred::prelude::Error),
    #[error(transparent)]
    ApalisRedisError(#[from] apalis_redis::RedisError),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let user_not_found_response = (
            StatusCode::NOT_FOUND,
            Json(ErrorDto {
                error: "User not found".to_string(),
            }),
        );

        match self {
            Error::AuthCsrfInvalidState => {
                debug!("Authentication error: {}", Error::AuthCsrfInvalidState);

                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorDto {
                        error: "There was an issue logging you in, please try again.".to_string(),
                    }),
                )
                    .into_response()
            }
            Error::AuthUserNotInSession => {
                debug!("Authentication error: {}", Error::AuthUserNotInSession);

                user_not_found_response.into_response()
            }
            Error::AuthUserNotInDatabase(user_id) => {
                debug!(
                    "Authentication error: {}",
                    Error::AuthUserNotInDatabase(user_id)
                );

                user_not_found_response.into_response()
            }
            err => {
                error!("Internal server error: {}", err);

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorDto {
                        error: "Internal server error".to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }
}
