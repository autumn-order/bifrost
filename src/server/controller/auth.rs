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
        controller::util::csrf::validate_csrf,
        error::Error,
        model::{
            app::AppState,
            session::{
                auth::SessionAuthCsrf, change_main::SessionUserChangeMain, user::SessionUserId,
            },
        },
        service::{
            auth::{callback::CallbackService, login::login_service},
            user::UserService,
        },
    },
};

pub static AUTH_TAG: &str = "auth";

#[derive(Deserialize)]
pub struct LoginParams {
    pub change_main: Option<bool>,
}

#[derive(Deserialize)]
pub struct CallbackParams {
    pub state: String,
    pub code: String,
}

/// Login route to initiate login with EVE Online
///
/// Creates a URL to login with EVE Online and redirects the user to that URL to begin the login process.
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
    let scopes = eve_esi::ScopeBuilder::new().build();

    if let Some(true) = params.0.change_main {
        SessionUserChangeMain::insert(&session, true).await?;
    }

    let login = login_service(&state.esi_client, scopes)?;

    SessionAuthCsrf::insert(&session, &login.state).await?;

    Ok(Redirect::temporary(&login.login_url))
}

/// Callback route user is redirected to after successful login at EVE Online's website
///
/// This route fetches & validates the user's token to access character information as well as
/// the access & refresh token for fetching data related to the requested scopes.
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
        SessionUserId::insert(&session, user_id).await?;
    }

    Ok(Redirect::permanent("/auth"))
}

/// Logs the user out by clearing their session
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
    if let Some(_) = maybe_user_id {
        session.clear().await;
    }

    Ok(Redirect::temporary("/"))
}

/// Returns information on the currently logged in user
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
    let user_service = UserService::new(&state.db, &state.esi_client);

    let user_id = SessionUserId::get(&session).await?;

    match user_id {
        Some(id) => match user_service.get_user(id).await? {
            Some(user) => Ok((StatusCode::OK, axum::Json(user)).into_response()),
            None => {
                // Clear session for user not found in database
                session.clear().await;

                tracing::warn!(
                    "Failed to find user ID {} in database despite having an active session;
                    cleared session for user, they will need to relog to fix",
                    id
                );

                Ok((
                    StatusCode::NOT_FOUND,
                    axum::Json(ErrorDto {
                        error: "User not found".to_string(),
                    }),
                )
                    .into_response())
            }
        },
        None => Ok((
            StatusCode::NOT_FOUND,
            axum::Json(ErrorDto {
                error: "User not found".to_string(),
            }),
        )
            .into_response()),
    }
}
