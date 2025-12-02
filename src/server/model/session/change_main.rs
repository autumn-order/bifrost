//! Change main character session data models.
//!
//! This module provides type-safe wrappers for storing and retrieving the "change main"
//! flag in the session. This flag is set during login initiation when a user wants to
//! change their main character, and is consumed during the OAuth callback to determine
//! whether the authenticated character should become the user's new main character.

use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::server::error::Error;

/// Session key for storing the change main character flag.
///
/// This constant defines the Redis key used to store the temporary flag indicating that
/// the user wants to change their main character during the current authentication flow.
/// The key is namespaced under "bifrost:user:" to avoid collisions with other session data.
pub const SESSION_USER_CHANGE_MAIN_KEY: &str = "bifrost:user:change_main";

/// Session wrapper for the change main character flag.
///
/// This struct wraps a boolean flag that indicates whether the OAuth callback should
/// update the user's main character to the authenticated character. The flag is set
/// during login initiation (when `?change_main=true` is passed) and consumed during
/// the OAuth callback. The wrapper implements `Default` for initialization and `Debug`
/// for diagnostics.
#[derive(Default, Deserialize, Serialize, Debug)]
pub struct SessionUserChangeMain(pub bool);

impl SessionUserChangeMain {
    /// Inserts the change main character flag into the session.
    ///
    /// Stores a boolean flag in the session indicating that the user wants to change their
    /// main character during the current authentication flow. This is typically set when
    /// the login endpoint is called with `?change_main=true`. The flag is consumed during
    /// the OAuth callback to determine if the authenticated character should become the
    /// user's new main character.
    ///
    /// # Arguments
    /// - `session` - User's session for storing the flag
    /// - `change_main` - Whether to change the main character (typically `true` when set)
    ///
    /// # Returns
    /// - `Ok(())` - Flag successfully stored in session
    /// - `Err(Error)` - Session storage failed (Redis error, serialization error)
    pub async fn insert(session: &Session, change_main: bool) -> Result<(), Error> {
        session
            .insert(
                SESSION_USER_CHANGE_MAIN_KEY,
                SessionUserChangeMain(change_main),
            )
            .await?;

        Ok(())
    }

    /// Removes and returns the change main character flag from the session.
    ///
    /// Retrieves the change main flag from the session store and removes it, ensuring the
    /// flag can only be used once for a single authentication flow. This method is called
    /// during OAuth callback processing to determine if the authenticated character should
    /// become the user's new main character. Returns `None` if no flag is present (normal
    /// authentication flow without main character change).
    ///
    /// # Arguments
    /// - `session` - User's session to remove the flag from
    ///
    /// # Returns
    /// - `Ok(Some(true))` - Change main flag was set, should update main character
    /// - `Ok(Some(false))` - Flag was set to false (unlikely but valid)
    /// - `Ok(None)` - No flag present, normal authentication flow
    /// - `Err(Error)` - Session operation failed (Redis error)
    pub async fn remove(session: &Session) -> Result<Option<bool>, Error> {
        Ok(session.remove(SESSION_USER_CHANGE_MAIN_KEY).await?)
    }
}
