//! User session retrieval utilities.
//!
//! This module provides functions to retrieve authenticated user information from the session
//! and database. It handles session validation, user lookup, and automatic session cleanup
//! when users are not found in the database.

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

/// Retrieves user information from session and database.
///
/// Extracts the user ID from the session, queries the database for the user's full information,
/// and returns it as a DTO. If the user ID exists in the session but the user is not found in
/// the database (e.g., the user was deleted), the session is automatically cleared to prevent
/// stale session state. This function is commonly used by protected endpoints that require an
/// authenticated user.
///
/// # Arguments
/// - `state` - Application state with database connection for user lookup
/// - `session` - User's session containing their user ID
///
/// # Returns
/// - `Ok(UserDto)` - User found, containing user ID and main character information (ID, name)
/// - `Err(Error::AuthError(AuthError::UserNotInSession))` - No user ID present in session
/// - `Err(Error::AuthError(AuthError::UserNotInDatabase))` - User ID exists in session but user not found in database (session is cleared)
/// - `Err(Error)` - Database query failure or session retrieval error
pub async fn get_user_from_session(state: &AppState, session: &Session) -> Result<UserDto, Error> {
    // Get user from session
    let Some(user_id) = SessionUserId::get(session).await? else {
        return Err(Error::AuthError(AuthError::UserNotInSession));
    };

    // Get user from database
    let Some(user) = UserService::new(&state.db).get_user(user_id).await? else {
        session.clear().await;

        tracing::debug!(
            "Session cleared for user ID {} with active session but was not found in database",
            user_id
        );

        return Err(Error::AuthError(AuthError::UserNotInDatabase(user_id)));
    };

    Ok(user)
}
