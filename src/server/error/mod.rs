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

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] ConfigError),
    #[error(transparent)]
    AuthError(#[from] AuthError),
    #[error(transparent)]
    EveError(#[from] EveError),
    #[error(transparent)]
    WorkerError(#[from] WorkerError),
    #[error("Failed to parse value: {0:?}")]
    ParseError(String),
    /// General not found error for edge cases within Bifrost's code
    #[error("Internal error with Bifrost's code, please open a GitHub issue as this indicates a bug: {0:?}")]
    InternalError(String),
    #[error(transparent)]
    EsiError(#[from] eve_esi::Error),
    #[error(transparent)]
    DbErr(#[from] sea_orm::DbErr),
    #[error(transparent)]
    SessionError(#[from] tower_sessions::session::Error),
    #[error(transparent)]
    SessionRedisError(#[from] tower_sessions_redis_store::fred::prelude::Error),
}

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

pub struct InternalServerError<E>(pub E);

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
