use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use serde::Deserialize;
use tower_sessions::Session;

use crate::server::service::auth::callback::callback_service;
use crate::{
    model::api::ErrorDto,
    server::model::{app::AppState, session::AuthLoginCsrf},
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
) -> Response {
    let csrf_state = match AuthLoginCsrf::get(&session).await {
        Ok(state) => match state {
            Some(state) => state,
            None => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorDto {
                        error: "Internal server error".to_string(),
                    }),
                )
                    .into_response();
            }
        },
        Err(err) => return err.into_response(),
    };

    if csrf_state != params.0.state {
        return (
            StatusCode::BAD_REQUEST,
            "There was an issue logging you in, please try again.",
        )
            .into_response();
    }

    match callback_service(&state.esi_client, params.0.code).await {
        Ok(character) => (StatusCode::OK, Json(character)).into_response(),
        Err(err) => err.into_response(),
    }
}
