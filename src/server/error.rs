use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use dioxus_logger::tracing::{debug, error};
use thiserror::Error;
use tower_sessions_redis_store::fred::error::RedisError;

use crate::model::api::ErrorDto;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to login user due to CSRF state present in session store but without a value")]
    AuthCsrfEmptySession,
    #[error("Failed to login user due to CSRF state mismatch")]
    AuthCsrfInvalidState,
    #[error(transparent)]
    EsiError(#[from] eve_esi::Error),
    #[error(transparent)]
    DbErr(#[from] sea_orm::DbErr),
    #[error(transparent)]
    SessionError(#[from] tower_sessions::session::Error),
    #[error(transparent)]
    SessionRedisError(#[from] RedisError),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let internal_server_error = (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorDto {
                error: "Internal server error".to_string(),
            }),
        );

        match self {
            Error::AuthCsrfEmptySession => {
                error!(
                    "Authentication-related internal server error: {}",
                    Error::AuthCsrfEmptySession
                );

                internal_server_error.into_response()
            }
            Error::AuthCsrfInvalidState => {
                debug!("Authentication error: {}", Error::AuthCsrfInvalidState);

                (
                    StatusCode::BAD_REQUEST,
                    "There was an issue logging you in, please try again.",
                )
                    .into_response()
            }
            Error::EsiError(err) => {
                error!("ESI-related internal server error: {}", err);

                internal_server_error.into_response()
            }
            Error::DbErr(err) => {
                error!("Database-related internal server error: {}", err);

                internal_server_error.into_response()
            }
            Error::SessionError(err) => {
                error!("Session-related internal server error: {}", err);

                internal_server_error.into_response()
            }
            Error::SessionRedisError(err) => {
                error!("Session-related internal server error: {}", err);

                internal_server_error.into_response()
            }
        }
    }
}
