use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::{
    model::api::ErrorDto,
    server::{error::Error, model::app::AppState, service::user::UserService},
};

/// Returns information on the user's main character and linked characters
///
/// # Responses
/// - 200 (Success): The user's main character and a Vec of linked characters
/// - 404 (Not Found): The provided user ID does not exist
/// - 500 (Internal Server Error): An error if there is a database-related issue
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
) -> Result<impl IntoResponse, Error> {
    let user_service = UserService::new(&state.db, &state.esi_client);

    match user_service.get_user(user_id).await? {
        Some(user) => Ok((StatusCode::OK, Json(user)).into_response()),
        None => Ok((
            StatusCode::NOT_FOUND,
            Json(ErrorDto {
                error: "User not found".to_string(),
            }),
        )
            .into_response()),
    }
}
