use axum::{extract::State, http::StatusCode, response::IntoResponse};
use tower_sessions::Session;

use crate::{
    model::{api::ErrorDto, user::CharacterDto},
    server::{
        controller::util::get_user::get_user_from_session, error::Error, model::app::AppState,
        service::user::user_character::UserCharacterService,
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
    let user = get_user_from_session(&state, &session).await?;

    let character_dtos = UserCharacterService::new(&state.db, &state.esi_client)
        .get_user_characters(user.id)
        .await?;

    Ok((StatusCode::OK, axum::Json(character_dtos)).into_response())
}
