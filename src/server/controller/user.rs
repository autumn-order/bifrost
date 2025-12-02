//! User controller endpoints.
//!
//! This module provides HTTP endpoints for user-related operations, such as retrieving
//! information about characters owned by the authenticated user. These endpoints require
//! an active session and return user-specific data.

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use tower_sessions::Session;

use crate::{
    model::{api::ErrorDto, user::CharacterDto},
    server::{
        controller::util::get_user::get_user_from_session, error::Error, model::app::AppState,
        service::user::user_character::UserCharacterService,
    },
};

/// OpenAPI tag for user-related endpoints.
pub static USER_TAG: &str = "user";

/// Retrieves all characters owned by the currently authenticated user.
///
/// Fetches the user ID from the session, queries the database for all characters associated
/// with that user account, and returns them as a list of character DTOs. Each character DTO
/// includes the character's ID, name, corporation, alliance, and other relevant information.
///
/// # Arguments
/// - `state` - Application state containing the database connection for character lookup
/// - `session` - User's session containing their user ID
///
/// # Returns
/// - `Ok(Vec<CharacterDto>)` - List of characters owned by the user (may be empty)
/// - `Err(Error)` - User not in session, not found in database, or database error
#[utoipa::path(
    get,
    path = "/api/user/characters",
    tag = USER_TAG,
    responses(
        (status = 200, description = "Success when retrieving user characters", body = Vec<CharacterDto>),
        (status = 404, description = "User not found", body = ErrorDto),
        (status = 500, description = "Internal server error", body = ErrorDto)
    ),
)]
pub async fn get_user_characters(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, Error> {
    let user = get_user_from_session(&state, &session).await?;

    let character_dtos = UserCharacterService::new(&state.db)
        .get_user_characters(user.id)
        .await?;

    Ok((StatusCode::OK, axum::Json(character_dtos)).into_response())
}
