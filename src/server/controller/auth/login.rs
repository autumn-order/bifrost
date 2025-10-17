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

/// Login route to initiate login with EVE Online
///
/// Creates a URL to login with EVE Online and redirects the user to that URL to begin the login process.
///
/// # Responses
/// - 307 (Redirect Temporary): Redirects user to a temporary URL to start the EVE Online login process
/// - 500 (Internal Server Error): An error if the ESI client is not properly configured for OAuth2
pub async fn login(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, Error> {
    let scopes = eve_esi::ScopeBuilder::new().build();

    let login = login_service(&state.esi_client, scopes)?;

    AuthLoginCsrf::insert(&session, &login.state).await?;

    Ok(Redirect::temporary(&login.login_url))
}
