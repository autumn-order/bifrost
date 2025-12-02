//! Authentication and authorization error types.
//!
//! This module defines errors related to user authentication, session management, CSRF
//! validation, and character ownership. Authentication errors are mapped to appropriate
//! HTTP status codes (400, 404, 500) based on the error type and include user-friendly
//! error messages suitable for API responses.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use dioxus_logger::tracing;
use thiserror::Error;

use crate::{model::api::ErrorDto, server::error::InternalServerError};

/// Authentication and authorization error type.
///
/// These errors occur during authentication flows (login, callback), session validation,
/// and character ownership checks. Each variant is mapped to an appropriate HTTP status
/// code and user-friendly error message in the `IntoResponse` implementation.
#[derive(Error, Debug)]
pub enum AuthError {
    /// User ID is not present in the session.
    ///
    /// This error occurs when a protected endpoint is accessed without an active session
    /// or when the session has expired. Results in a 404 Not Found response.
    #[error("User ID is not present in session")]
    UserNotInSession,

    /// User ID exists in session but user not found in database.
    ///
    /// This error indicates the session references a user that no longer exists in the
    /// database (e.g., the user was deleted). The session is automatically cleared when
    /// this error is encountered. Results in a 404 Not Found response.
    #[error("User ID {0:?} not found in database")]
    UserNotInDatabase(i32),

    /// CSRF state validation failed during OAuth callback.
    ///
    /// The CSRF state token in the OAuth callback URL does not match the token stored
    /// in the session, indicating a potential CSRF attack or an invalid callback request.
    /// Results in a 400 Bad Request response.
    #[error("Failed to login user due to CSRF state mismatch")]
    CsrfValidationFailed,

    /// CSRF state is present in session but has no value.
    ///
    /// This is an internal error indicating session data corruption or serialization
    /// issues. Results in a 400 Bad Request response.
    #[error("Failed to login user due to CSRF state present in session store but without a value")]
    CsrfMissingValue,

    /// Character is owned by a different user.
    ///
    /// This error occurs when attempting to change a user's main character to a character
    /// owned by another user. Results in a 400 Bad Request response with a generic
    /// "Invalid character selection" message.
    #[error("Character is owned by another user")]
    CharacterOwnedByAnotherUser,

    /// Character is not owned by any user.
    ///
    /// This error occurs when attempting to change a user's main character to a character
    /// that exists in the database but is not owned by any user. Results in a 400 Bad
    /// Request response with a generic "Invalid character selection" message.
    #[error("Character is not owned by any user")]
    CharacterNotOwned,

    /// Character not found in database.
    ///
    /// This error occurs when a character lookup fails, typically during authentication
    /// or character ownership operations. Results in a 500 Internal Server Error response.
    #[error("Character not found in database")]
    CharacterNotFound,
}

impl AuthError {
    /// Creates a 404 Not Found response for user lookup failures.
    ///
    /// This helper method generates a consistent error response for both `UserNotInSession`
    /// and `UserNotInDatabase` errors, providing a user-friendly "User not found" message.
    ///
    /// # Returns
    /// A 404 Not Found response with a "User not found" error message
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

/// Converts authentication errors into HTTP responses.
///
/// Maps authentication errors to appropriate HTTP status codes and user-friendly error messages:
/// - `UserNotInSession` / `UserNotInDatabase` → 404 Not Found with "User not found"
/// - `CsrfValidationFailed` / `CsrfMissingValue` → 400 Bad Request with "There was an issue logging you in"
/// - `CharacterOwnedByAnotherUser` / `CharacterNotOwned` → 400 Bad Request with "Invalid character selection"
/// - Other errors → 500 Internal Server Error with generic message
///
/// All errors are logged at debug level for diagnostics while keeping client-facing messages
/// generic to avoid information leakage.
///
/// # Returns
/// - 400 Bad Request - For CSRF failures and invalid character operations
/// - 404 Not Found - For missing users
/// - 500 Internal Server Error - For unexpected authentication errors
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
