use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use dioxus_logger::tracing::error;
use thiserror::Error;

use crate::model::api::ErrorDto;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    EsiError(#[from] eve_esi::Error),
    #[error(transparent)]
    SessionError(#[from] tower_sessions::session::Error),
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
            Error::EsiError(err) => {
                error!("Internal server error: {}", err);

                internal_server_error.into_response()
            }
            Error::SessionError(err) => {
                error!("Internal server error: {}", err);

                internal_server_error.into_response()
            }
        }
    }
}
