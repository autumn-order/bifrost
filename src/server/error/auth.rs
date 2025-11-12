use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use dioxus_logger::tracing;
use thiserror::Error;

use crate::{model::api::ErrorDto, server::error::InternalServerError};

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("User ID is not present in session")]
    UserNotInSession,
    #[error("User ID {0:?} not found in database despite having an active session")]
    UserNotInDatabase(i32),
    #[error("Failed to login user due to CSRF state mismatch")]
    CsrfValidationFailed,
    #[error("Failed to login user due to CSRF state present in session store but without a value")]
    CsrfMissingValue,
    #[error("Character is owned by another user")]
    CharacterOwnedByAnotherUser,
    #[error("Character is not owned by any user")]
    CharacterNotOwned,
    #[error("Character not found in database")]
    CharacterNotFound,
}

impl AuthError {
    fn user_not_found() -> Response {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorDto {
                error: "User not found".to_string(),
            }),
        )
            .into_response()
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            Self::UserNotInSession => {
                tracing::debug!("{}", Self::UserNotInSession);

                Self::user_not_found()
            }
            Self::UserNotInDatabase(user_id) => {
                tracing::debug!(
                    user_id = %user_id,
                    "{}",
                    self
                );

                Self::user_not_found()
            }
            Self::CsrfValidationFailed => {
                tracing::debug!("{}", Self::CsrfMissingValue);

                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorDto {
                        error: "There was an issue logging you in, please try again.".to_string(),
                    }),
                )
                    .into_response()
            }
            Self::CharacterOwnedByAnotherUser => {
                tracing::debug!("{}", self);

                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorDto {
                        error: "Invalid character selection".to_string(),
                    }),
                )
                    .into_response()
            }
            Self::CharacterNotOwned => {
                tracing::debug!("{}", self);

                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorDto {
                        error: "Invalid character selection".to_string(),
                    }),
                )
                    .into_response()
            }
            err => InternalServerError(err).into_response(),
        }
    }
}
