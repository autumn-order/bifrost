use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use tower_sessions::Session;

use crate::server::{
    error::Error,
    model::{app::AppState, session::AuthLoginCsrf},
    service::auth::login::login_service,
};

pub async fn login(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, Error> {
    let scopes = eve_esi::ScopeBuilder::new().build();

    let login = login_service(&state.esi_client, scopes)?;

    AuthLoginCsrf::insert(&session, login.state).await?;

    Ok(Redirect::temporary(&login.login_url))
}
