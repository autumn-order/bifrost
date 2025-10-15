use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::{Redirect, Response};
use axum::Json;
use tower_sessions::Session;

use crate::server::model::session::{AuthLoginCsrf, AUTH_LOGIN_CSRF_KEY};
use crate::{model::api::ErrorDto, server::model::app::AppState};

pub async fn login(State(state): State<AppState>, session: Session) -> Response {
    let scopes = eve_esi::ScopeBuilder::new().build();

    let login = match state.esi_client.oauth2().login_url(scopes) {
        Ok(login) => login,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorDto {
                    error: "Internal sever error".to_string(),
                }),
            )
                .into_response();
        }
    };

    session
        .insert(AUTH_LOGIN_CSRF_KEY, AuthLoginCsrf(login.state))
        .await
        .unwrap();

    Redirect::temporary(&login.login_url).into_response()
}
