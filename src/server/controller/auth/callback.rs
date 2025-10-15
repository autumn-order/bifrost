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

pub async fn callback(
    State(state): State<AppState>,
    session: Session,
    params: Query<CallbackParams>,
) -> Result<impl IntoResponse, Error> {
    validate_csrf(&session, &params.0.state).await?;

    let character = callback_service(&state.esi_client, params.0.code).await?;

    Ok((axum::http::StatusCode::OK, Json(character)))
}
