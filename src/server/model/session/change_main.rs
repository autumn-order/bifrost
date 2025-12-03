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

#[cfg(test)]
mod tests {
    use super::*;

    mod insert {
        use super::*;
        use bifrost_test_utils::prelude::*;

        /// Tests successful insertion of change_main flag set to true.
        ///
        /// Verifies that a change_main flag with value true can be stored in the session
        /// without errors.
        ///
        /// Expected: Ok(())
        #[tokio::test]
        async fn inserts_true_flag_into_session() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionUserChangeMain::insert(&test.session, true).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Tests successful insertion of change_main flag set to false.
        ///
        /// Verifies that a change_main flag with value false can be stored in the session
        /// without errors.
        ///
        /// Expected: Ok(())
        #[tokio::test]
        async fn inserts_false_flag_into_session() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionUserChangeMain::insert(&test.session, false).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Tests that inserted true flag can be retrieved with correct value.
        ///
        /// Verifies that the change_main flag stored in the session matches the original
        /// value when retrieved, ensuring data integrity during storage.
        ///
        /// Expected: Ok(()) and retrieved value is true
        #[tokio::test]
        async fn inserted_true_flag_is_retrievable() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let insert_result = SessionUserChangeMain::insert(&test.session, true).await;
            assert!(insert_result.is_ok());

            let remove_result = SessionUserChangeMain::remove(&test.session).await;
            assert!(remove_result.is_ok());
            assert_eq!(remove_result.unwrap(), Some(true));

            Ok(())
        }

        /// Tests that inserted false flag can be retrieved with correct value.
        ///
        /// Verifies that a false change_main flag stored in the session matches the
        /// original value when retrieved.
        ///
        /// Expected: Ok(()) and retrieved value is false
        #[tokio::test]
        async fn inserted_false_flag_is_retrievable() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let insert_result = SessionUserChangeMain::insert(&test.session, false).await;
            assert!(insert_result.is_ok());

            let remove_result = SessionUserChangeMain::remove(&test.session).await;
            assert!(remove_result.is_ok());
            assert_eq!(remove_result.unwrap(), Some(false));

            Ok(())
        }

        /// Tests that inserting a new flag overwrites the previous one.
        ///
        /// Verifies that when a change_main flag is inserted multiple times, the latest
        /// value overwrites the previous one.
        ///
        /// Expected: Ok(()) and retrieved value matches the latest inserted value
        #[tokio::test]
        async fn overwrites_existing_flag() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let _ = SessionUserChangeMain::insert(&test.session, false)
                .await
                .unwrap();
            let _ = SessionUserChangeMain::insert(&test.session, true)
                .await
                .unwrap();

            let result = SessionUserChangeMain::remove(&test.session).await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), Some(true));

            Ok(())
        }
    }

    mod remove {
        use super::*;
        use bifrost_test_utils::prelude::*;

        /// Tests successful removal of change_main flag from session.
        ///
        /// Verifies that a stored change_main flag can be removed from the session
        /// successfully, returning Ok(Some(true)).
        ///
        /// Expected: Ok(Some(true))
        #[tokio::test]
        async fn removes_true_flag_from_session() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let _ = SessionUserChangeMain::insert(&test.session, true)
                .await
                .unwrap();

            let result = SessionUserChangeMain::remove(&test.session).await;

            assert!(result.is_ok());
            let removed_value = result.unwrap();
            assert_eq!(removed_value, Some(true));

            Ok(())
        }

        /// Tests successful removal of false flag from session.
        ///
        /// Verifies that a stored change_main flag with value false can be removed
        /// and returns the correct value.
        ///
        /// Expected: Ok(Some(false))
        #[tokio::test]
        async fn removes_false_flag_from_session() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let _ = SessionUserChangeMain::insert(&test.session, false)
                .await
                .unwrap();

            let result = SessionUserChangeMain::remove(&test.session).await;

            assert!(result.is_ok());
            let removed_value = result.unwrap();
            assert_eq!(removed_value, Some(false));

            Ok(())
        }

        /// Tests removal when flag is not present in session.
        ///
        /// Verifies that `remove` returns Ok(None) when attempting to remove a
        /// change_main flag from an empty session (normal authentication flow).
        ///
        /// Expected: Ok(None)
        #[tokio::test]
        async fn returns_none_when_flag_missing() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionUserChangeMain::remove(&test.session).await;

            assert!(result.is_ok());
            let removed_value = result.unwrap();
            assert_eq!(removed_value, None);

            Ok(())
        }

        /// Tests that flag is actually removed after removal.
        ///
        /// Verifies that after calling `remove`, the change_main flag is no longer
        /// present in the session and subsequent attempts to retrieve it return None.
        ///
        /// Expected: First removal succeeds with Some(true), second attempt returns None
        #[tokio::test]
        async fn flag_not_retrievable_after_removal() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let _ = SessionUserChangeMain::insert(&test.session, true)
                .await
                .unwrap();

            let first_remove = SessionUserChangeMain::remove(&test.session).await;
            assert!(first_remove.is_ok());
            assert_eq!(first_remove.unwrap(), Some(true));

            // Second removal should return None
            let second_remove = SessionUserChangeMain::remove(&test.session).await;
            assert!(second_remove.is_ok());
            assert_eq!(second_remove.unwrap(), None);

            Ok(())
        }

        /// Tests that remove is idempotent (calling remove twice returns None the second time).
        ///
        /// Verifies that after successfully removing a change_main flag, a second removal
        /// attempt returns Ok(None).
        ///
        /// Expected: First removal returns Some(value), second removal returns None
        #[tokio::test]
        async fn second_removal_returns_none() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let _ = SessionUserChangeMain::insert(&test.session, true)
                .await
                .unwrap();

            let first_remove = SessionUserChangeMain::remove(&test.session).await;
            assert!(first_remove.is_ok());
            assert!(first_remove.unwrap().is_some());

            let second_remove = SessionUserChangeMain::remove(&test.session).await;
            assert!(second_remove.is_ok());
            assert_eq!(second_remove.unwrap(), None);

            Ok(())
        }
    }
}
