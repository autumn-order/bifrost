use axum::extract::State;
use axum::response::IntoResponse;
use axum::response::{Redirect, Response};
use tower_sessions::Session;

use crate::server::model::app::AppState;
use crate::server::model::session::AuthLoginCsrf;
use crate::server::service::auth::login::login_service;

pub async fn login(State(state): State<AppState>, session: Session) -> Response {
    let scopes = eve_esi::ScopeBuilder::new().build();

    let login = match login_service(&state.esi_client, scopes) {
        Ok(login) => login,
        Err(err) => return err.into_response(),
    };

    match AuthLoginCsrf::insert(&session, login.state).await {
        Ok(_) => Redirect::temporary(&login.login_url).into_response(),
        Err(err) => return err.into_response(),
    }
}
