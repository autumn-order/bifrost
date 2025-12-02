//! Authentication controller endpoints.
//!
//! This module provides HTTP endpoints for EVE Online SSO authentication flow, including
//! login initiation, OAuth callback handling, logout, and retrieving the current user's
//! information. It manages session state for CSRF protection and user identity tracking.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use dioxus_logger::tracing;
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    model::{api::ErrorDto, user::UserDto},
    server::{
        controller::util::{csrf::validate_csrf, get_user::get_user_from_session},
        error::Error,
        model::{
            app::AppState,
            session::{
                auth::SessionAuthCsrf, change_main::SessionUserChangeMain, user::SessionUserId,
            },
        },
        service::auth::{callback::CallbackService, login::LoginService},
    },
};

/// OpenAPI tag for authentication-related endpoints.
pub static AUTH_TAG: &str = "auth";

/// Query parameters for the login endpoint.
///
/// # Fields
/// - `change_main` - Optional flag to indicate if the login should change the user's main character
#[derive(Deserialize)]
pub struct LoginParams {
    /// If true, the authenticated character will become the user's main character.
    pub change_main: Option<bool>,
}

/// Query parameters for the OAuth callback endpoint.
///
/// These parameters are provided by EVE Online's SSO server after successful authentication.
///
/// # Fields
/// - `state` - CSRF protection token that must match the value stored in the session
/// - `code` - Authorization code used to exchange for access tokens
#[derive(Deserialize)]
pub struct CallbackParams {
    /// CSRF state token to be validated against the session value.
    pub state: String,
    /// Authorization code from EVE Online SSO for token exchange.
    pub code: String,
}

/// Initiates EVE Online SSO authentication flow.
///
/// Generates an EVE Online SSO login URL with CSRF protection and redirects the user to it.
/// The CSRF state token is stored in the session for validation during the callback. If the
/// `change_main` parameter is set, the session is flagged so that the authenticated character
/// will become the user's new main character after successful login.
///
/// # Arguments
/// - `state` - Application state containing the ESI client for login URL generation
/// - `session` - User's session for storing CSRF token and change_main flag
/// - `params` - Query parameters, optionally including `change_main` flag
///
/// # Returns
/// - `Ok(Redirect)` - 307 temporary redirect to EVE Online SSO login page
/// - `Err(Error)` - Failed to generate login URL or store session data
#[utoipa::path(
    get,
    path = "/api/auth/login",
    tag = AUTH_TAG,
    responses(
        (status = 307, description = "Redirect to EVE Online login URL"),
        (status = 500, description = "Internal server error", body = ErrorDto)
    ),
    params(
        ("change_main" = Option<bool>, Query, description = "If true, change logged in user's main to character"),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    session: Session,
    params: Query<LoginParams>,
) -> Result<impl IntoResponse, Error> {
    let login_service = LoginService::new(&state.esi_client);
    let scopes = eve_esi::ScopeBuilder::new().build();

    if let Some(true) = params.0.change_main {
        SessionUserChangeMain::insert(&session, true).await?;
    }

    let login = login_service.generate_login_url(scopes)?;

    SessionAuthCsrf::insert(&session, &login.state).await?;

    Ok(Redirect::temporary(&login.login_url))
}

/// Handles OAuth callback from EVE Online SSO after successful authentication.
///
/// Validates the CSRF state token, exchanges the authorization code for access/refresh tokens,
/// verifies the character JWT token, and either creates a new user or associates the character
/// with an existing user. If `change_main` was set during login, the authenticated character
/// becomes the user's new main character. The user ID is stored in the session for subsequent
/// requests.
///
/// # Arguments
/// - `state` - Application state containing database and ESI client for callback processing
/// - `session` - User's session for CSRF validation and storing user ID
/// - `params` - Query parameters containing CSRF state and authorization code from EVE SSO
///
/// # Returns
/// - `Ok(Redirect)` - 308 permanent redirect to `/auth` after successful authentication
/// - `Err(Error)` - CSRF validation failed, token exchange failed, or database error
#[utoipa::path(
    get,
    path = "/api/auth/callback",
    tag = AUTH_TAG,
    responses(
        (status = 307, description = "Redirect to user information API route"),
        (status = 400, description = "CSRF state in URL does not match state in session", body = ErrorDto),
        (status = 500, description = "Internal server error", body = ErrorDto)
    ),
    params(
        ("state" = String, Query, description = "CSRF state to be validated during callback against state stored in session"),
        ("code" = String, Query, description = "Authorization code used to fetch a JWT token from EVE Online authentication servers")
    )
)]
pub async fn callback(
    State(state): State<AppState>,
    session: Session,
    params: Query<CallbackParams>,
) -> Result<impl IntoResponse, Error> {
    let callback_service = CallbackService::new(&state.db, &state.esi_client);

    validate_csrf(&session, &params.0.state).await?;

    let maybe_user_id = SessionUserId::get(&session).await?;
    let change_main = SessionUserChangeMain::remove(&session).await?;

    let user_id = callback_service
        .handle_callback(&params.0.code, maybe_user_id, change_main)
        .await?;

    if maybe_user_id.is_none() {
        tracing::trace!(
            "Inserting user ID {} into session after successful callback",
            user_id
        );

        SessionUserId::insert(&session, user_id).await?;
    }

    Ok(Redirect::permanent("/auth"))
}

/// Logs out the current user by clearing their session data.
///
/// Removes all session data including user ID, effectively logging the user out. Only attempts
/// to clear the session if a user ID is present, avoiding errors when clearing empty sessions.
/// After logout, redirects to the home page.
///
/// # Arguments
/// - `session` - User's session to be cleared
///
/// # Returns
/// - `Ok(Redirect)` - 307 temporary redirect to home page (`/`)
/// - `Err(Error)` - Failed to retrieve or clear session data
#[utoipa::path(
    get,
    path = "/api/auth/logout",
    tag = AUTH_TAG,
    responses(
        (status = 307, description = "Redirect to login API route after logout"),
        (status = 500, description = "Internal server error", body = ErrorDto)
    )
)]
pub async fn logout(session: Session) -> Result<impl IntoResponse, Error> {
    let maybe_user_id = SessionUserId::get(&session).await?;

    // Only clear session if there is actually a user in session
    //
    // This avoids a 500 internal error response that occurs when trying
    // to clear sessions which don't exist
    if maybe_user_id.is_some() {
        session.clear().await;
    }

    Ok(Redirect::temporary("/"))
}

/// Retrieves information about the currently authenticated user.
///
/// Fetches the user ID from the session and queries the database for complete user information,
/// including their main character details. Returns a 404 error if the user is not found in the
/// database (which may indicate their account was deleted or the session is stale).
///
/// # Arguments
/// - `state` - Application state containing the database connection for user lookup
/// - `session` - User's session containing their user ID
///
/// # Returns
/// - `Ok(UserDto)` - User information including ID and main character details
/// - `Err(Error)` - User not in session, not found in database, or database error
#[utoipa::path(
    get,
    path = "/api/auth/user",
    tag = AUTH_TAG,
    responses(
        (status = 200, description = "Success when retrieving user information", body = UserDto),
        (status = 404, description = "User not found", body = ErrorDto),
        (status = 500, description = "Internal server error", body = ErrorDto)
    ),
)]
pub async fn get_user(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, Error> {
    let user = get_user_from_session(&state, &session).await?;

    Ok((StatusCode::OK, axum::Json(user)).into_response())
}
