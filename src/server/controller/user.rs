use axum::{extract::State, http::StatusCode, response::IntoResponse};
use dioxus_logger::tracing;
use tower_sessions::Session;

use crate::{
    model::{api::ErrorDto, user::CharacterDto},
    server::{
        error::Error,
        model::{app::AppState, session::user::SessionUserId},
        service::user::{user_character::UserCharacterService, UserService},
    },
};

pub static USER_TAG: &str = "user";

/// Get all characters owned by logged in user
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
    let user_service = UserService::new(&state.db, &state.esi_client);

    let user_id = SessionUserId::get(&session).await?;

    let user_id = if let Some(user_id) = user_id {
        user_id
    } else {
        return Ok((
            StatusCode::NOT_FOUND,
            axum::Json(ErrorDto {
                error: "User not found".to_string(),
            }),
        )
            .into_response());
    };

    let user = if let Some(user) = user_service.get_user(user_id).await? {
        user
    } else {
        // Clear session for user not found in database
        session.clear().await;

        tracing::warn!(
            "Failed to find user ID {} in database despite having an active session;
            cleared session for user, they will need to relog to fix",
            user_id
        );

        return Ok((
            StatusCode::NOT_FOUND,
            axum::Json(ErrorDto {
                error: "User not found".to_string(),
            }),
        )
            .into_response());
    };

    let character_dtos = UserCharacterService::new(&state.db, &state.esi_client)
        .get_user_characters(user.id)
        .await?;

    Ok((StatusCode::OK, axum::Json(character_dtos)).into_response())
}
