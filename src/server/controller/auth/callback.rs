use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::server::{
    controller::auth::csrf::validate_csrf, error::Error, model::app::AppState,
    service::auth::callback::callback_service,
};

#[derive(Deserialize)]
pub struct CallbackParams {
    pub state: String,
    pub code: String,
}

/// Callback route user is redirected to after successful login at EVE Online's website
///
/// This route fetches & validates the user's token to access character information as well as
/// the access & refresh token for fetching data related to the requested scopes.
///
/// # Responses
/// - 200 (Success): Successful callback, returns character ID & name
/// - 400 (Bad Request): Failed to validate CSRF state due mismatch with the CSRF state stored in session
/// - 500 (Internal Server Error): An error occurred related to JWT token validation
pub async fn callback(
    State(state): State<AppState>,
    session: Session,
    params: Query<CallbackParams>,
) -> Result<impl IntoResponse, Error> {
    validate_csrf(&session, &params.0.state).await?;

    let character = callback_service(&state.esi_client, &params.0.code).await?;

    Ok((axum::http::StatusCode::OK, Json(character)))
}
