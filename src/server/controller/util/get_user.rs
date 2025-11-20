use dioxus_logger::tracing;
use tower_sessions::Session;

use crate::{
    model::user::UserDto,
    server::{
        error::{auth::AuthError, Error},
        model::{app::AppState, session::user::SessionUserId},
        service::user::UserService,
    },
};

/// Retrieves user information from session and then from database
///
/// # Arguments
/// - `state`: Application state with database connection & ESI client
/// - `session`: The user's session
///
/// # Returns
/// - `Ok(`UserDto)`: User found, containing user ID and main character ID & name
/// - `Err(Error::AuthUserNotInSession)`: User ID not present in session
/// - `Err(Error::AuthUserNotInDatabase)`: User ID exists in session but not found in database (session is cleared)
/// - `Err(Error)`: Internal errors (database query failures, session errors, etc.)
pub async fn get_user_from_session(state: &AppState, session: &Session) -> Result<UserDto, Error> {
    // Get user from session
    let Some(user_id) = SessionUserId::get(&session).await? else {
        return Err(Error::AuthError(AuthError::UserNotInSession));
    };

    // Get user from database
    let Some(user) = UserService::new(state.db.clone(), state.esi_client.clone())
        .get_user(user_id)
        .await?
    else {
        session.clear().await;

        tracing::debug!(
            "Session cleared for user ID {} with active session but was not found in database",
            user_id
        );

        return Err(Error::AuthError(AuthError::UserNotInDatabase(user_id)));
    };

    Ok(user)
}
