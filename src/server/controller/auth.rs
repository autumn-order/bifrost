use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::server::{
    controller::util::csrf::validate_csrf,
    error::Error,
    model::{
        app::AppState,
        session::{auth::SessionAuthCsrf, user::SessionUserId},
    },
    service::auth::{callback::CallbackService, login::login_service},
};

#[derive(Deserialize)]
pub struct CallbackParams {
    pub state: String,
    pub code: String,
}

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

    SessionAuthCsrf::insert(&session, &login.state).await?;

    Ok(Redirect::temporary(&login.login_url))
}

/// Callback route user is redirected to after successful login at EVE Online's website
///
/// This route fetches & validates the user's token to access character information as well as
/// the access & refresh token for fetching data related to the requested scopes.
///
/// # Responses
/// - 200 (Success): Successful callback, returns user ID
/// - 400 (Bad Request): Failed to validate CSRF state due mismatch with the CSRF state stored in session
/// - 500 (Internal Server Error): An error occurred related to JWT token validation, an ESI request, or
///   a database-related error
pub async fn callback(
    State(state): State<AppState>,
    session: Session,
    params: Query<CallbackParams>,
) -> Result<impl IntoResponse, Error> {
    let callback_service = CallbackService::new(&state.db, &state.esi_client);

    validate_csrf(&session, &params.0.state).await?;

    let maybe_user_id = SessionUserId::get(&session).await?;

    let user_id = callback_service
        .handle_callback(&params.0.code, maybe_user_id)
        .await?;

    if maybe_user_id.is_none() {
        SessionUserId::insert(&session, user_id).await?;
    }

    Ok(Redirect::temporary(&format!("/user/{}", user_id)))
}
