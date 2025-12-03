//! Authentication session data models.
//!
//! This module provides type-safe wrappers for storing and retrieving CSRF tokens in the
//! session during OAuth authentication flows. CSRF tokens are generated during login
//! initiation, stored in the session, and validated during the OAuth callback to prevent
//! Cross-Site Request Forgery attacks.

use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::server::error::{auth::AuthError, Error};

/// Session key for storing CSRF state token.
///
/// This constant defines the Redis key used to store the CSRF state token during OAuth
/// authentication flows. The key is namespaced under "bifrost:auth:" to avoid collisions
/// with other session data.
pub const SESSION_AUTH_CSRF_KEY: &str = "bifrost:auth:csrf";

/// Session wrapper for CSRF state token storage.
///
/// This struct wraps the CSRF state token as a string for serialization to the session store.
/// CSRF tokens are randomly generated during login initiation and validated during the OAuth
/// callback to ensure the authentication request originated from this application. The wrapper
/// implements `Default` for initialization and `Debug` for diagnostics.
#[derive(Default, Deserialize, Serialize, Debug)]
pub struct SessionAuthCsrf(pub String);

impl SessionAuthCsrf {
    /// Inserts the CSRF state token into the session.
    ///
    /// Stores the CSRF state token in the session during OAuth login initiation. This token
    /// will be validated against the state parameter in the OAuth callback URL to prevent
    /// CSRF attacks. The token is stored under `SESSION_AUTH_CSRF_KEY` and persisted to the
    /// Redis session store.
    ///
    /// # Arguments
    /// - `session` - User's session for storing the CSRF token
    /// - `state` - CSRF state token to store (randomly generated string)
    ///
    /// # Returns
    /// - `Ok(())` - CSRF token successfully stored in session
    /// - `Err(Error)` - Session storage failed (Redis error, serialization error)
    pub async fn insert(session: &Session, state: &str) -> Result<(), Error> {
        session
            .insert(SESSION_AUTH_CSRF_KEY, SessionAuthCsrf(state.to_string()))
            .await?;

        Ok(())
    }

    /// Retrieves the CSRF state token from the session.
    ///
    /// Fetches the CSRF state token from the session store without removing it. Returns an
    /// error if the CSRF token is not present in the session, which may indicate an invalid
    /// or expired session. This method is primarily used for validation purposes.
    ///
    /// # Arguments
    /// - `session` - User's session to retrieve the CSRF token from
    ///
    /// # Returns
    /// - `Ok(String)` - CSRF token found and retrieved successfully
    /// - `Err(Error::AuthError(AuthError::CsrfMissingValue))` - No CSRF token in session
    /// - `Err(Error)` - Session retrieval failed (Redis error)
    pub async fn get(session: &Session) -> Result<String, Error> {
        match session.get(SESSION_AUTH_CSRF_KEY).await? {
            Some(csrf) => Ok(csrf),
            None => Err(AuthError::CsrfMissingValue.into()),
        }
    }

    /// Removes and returns the CSRF state token from the session.
    ///
    /// Retrieves the CSRF state token from the session store and removes it, ensuring the
    /// token can only be used once (preventing replay attacks). This method is called during
    /// OAuth callback validation. Returns an error if no CSRF token is present in the session.
    ///
    /// # Arguments
    /// - `session` - User's session to remove the CSRF token from
    ///
    /// # Returns
    /// - `Ok(Some(String))` - CSRF token found, removed, and returned
    /// - `Err(Error::AuthError(AuthError::CsrfMissingValue))` - No CSRF token in session
    /// - `Err(Error)` - Session operation failed (Redis error)
    pub async fn remove(session: &Session) -> Result<Option<String>, Error> {
        match session.remove(SESSION_AUTH_CSRF_KEY).await? {
            Some(csrf) => Ok(csrf),
            None => Err(AuthError::CsrfMissingValue.into()),
        }
    }
}

#[cfg(test)]
mod tests {

    mod insert {
        use bifrost_test_utils::prelude::*;

        use crate::server::model::session::auth::SessionAuthCsrf;

        #[tokio::test]
        /// Tests successful insertion of CSRF token into session.
        ///
        /// Verifies that a CSRF state token can be stored in the session without errors.
        ///
        /// Expected: Ok(())
        async fn inserts_csrf_into_session() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionAuthCsrf::insert(&test.session, "string").await;

            assert!(result.is_ok());

            Ok(())
        }

        #[tokio::test]
        /// Tests that inserted CSRF token can be retrieved with correct value.
        ///
        /// Verifies that the CSRF token stored in the session matches the original value
        /// when retrieved, ensuring data integrity during storage.
        ///
        /// Expected: Ok(()) and retrieved value matches inserted value
        async fn inserted_csrf_is_retrievable() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let state = "test_csrf_token_12345";

            let insert_result = SessionAuthCsrf::insert(&test.session, state).await;
            assert!(insert_result.is_ok());

            let get_result = SessionAuthCsrf::get(&test.session).await;
            assert!(get_result.is_ok());
            assert_eq!(get_result.unwrap(), state);

            Ok(())
        }

        #[tokio::test]
        /// Tests that inserting a new CSRF token overwrites the previous one.
        ///
        /// Verifies that when a CSRF token is inserted multiple times, the latest value
        /// overwrites the previous one, ensuring only the most recent token is stored.
        ///
        /// Expected: Ok(()) and retrieved value matches the latest inserted value
        async fn overwrites_existing_csrf() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let first_state = "first_token";
            let second_state = "second_token";

            let _ = SessionAuthCsrf::insert(&test.session, first_state)
                .await
                .unwrap();
            let _ = SessionAuthCsrf::insert(&test.session, second_state)
                .await
                .unwrap();

            let result = SessionAuthCsrf::get(&test.session).await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), second_state);

            Ok(())
        }

        #[tokio::test]
        /// Tests insertion of empty string as CSRF token.
        ///
        /// Verifies that an empty string can be stored as a CSRF token (edge case),
        /// though this would not be used in production.
        ///
        /// Expected: Ok(())
        async fn inserts_empty_string() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionAuthCsrf::insert(&test.session, "").await;

            assert!(result.is_ok());

            Ok(())
        }
    }

    mod get {
        use bifrost_test_utils::prelude::*;

        use crate::server::{
            error::{auth::AuthError, Error},
            model::session::auth::SessionAuthCsrf,
        };

        /// Tests successful retrieval of CSRF token from session.
        ///
        /// Verifies that a stored CSRF token can be retrieved correctly and matches
        /// the original value that was inserted.
        ///
        /// Expected: Ok(state_string)
        #[tokio::test]
        async fn retrieves_csrf_from_session() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let state = "string";
            let _ = SessionAuthCsrf::insert(&test.session, state).await.unwrap();

            let result = SessionAuthCsrf::get(&test.session).await;

            assert!(result.is_ok());
            let result_state = result.unwrap();
            assert_eq!(result_state, state.to_string());

            Ok(())
        }

        /// Tests error when CSRF token is not present in session.
        ///
        /// Verifies that `get` returns a `CsrfMissingValue` error when attempting to
        /// retrieve a CSRF token from an empty session.
        ///
        /// Expected: Err(Error::AuthError(AuthError::CsrfMissingValue))
        #[tokio::test]
        async fn fails_when_csrf_missing() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionAuthCsrf::get(&test.session).await;

            // Should error due to state not being present in session
            assert!(result.is_err());
            assert!(matches!(
                result,
                Err(Error::AuthError(AuthError::CsrfMissingValue))
            ));

            Ok(())
        }
    }

    mod remove {
        use bifrost_test_utils::prelude::*;

        use crate::server::{
            error::{auth::AuthError, Error},
            model::session::auth::SessionAuthCsrf,
        };

        /// Tests successful removal of CSRF token from session.
        ///
        /// Verifies that a stored CSRF token can be removed from the session successfully,
        /// returning `Ok(Some(state))`.
        ///
        /// Expected: Ok(Some(state))
        #[tokio::test]
        async fn removes_csrf_from_session() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let _ = SessionAuthCsrf::insert(&test.session, "state")
                .await
                .unwrap();

            let result = SessionAuthCsrf::remove(&test.session).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Tests that removed CSRF token returns the correct value.
        ///
        /// Verifies that the value returned by `remove` matches the original value
        /// that was stored in the session.
        ///
        /// Expected: Ok(Some(state)) where state matches the inserted value
        #[tokio::test]
        async fn returns_correct_value_on_removal() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let state = "test_state_value";
            let _ = SessionAuthCsrf::insert(&test.session, state).await.unwrap();

            let result = SessionAuthCsrf::remove(&test.session).await;

            assert!(result.is_ok());
            let removed_value = result.unwrap();
            assert!(removed_value.is_some());
            assert_eq!(removed_value.unwrap(), state);

            Ok(())
        }

        /// Tests that CSRF token is actually removed after removal.
        ///
        /// Verifies that after calling `remove`, the CSRF token is no longer present
        /// in the session and subsequent attempts to retrieve it fail.
        ///
        /// Expected: First removal succeeds, second attempt fails with CsrfMissingValue
        #[tokio::test]
        async fn csrf_not_retrievable_after_removal() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let state = "state_to_remove";
            let _ = SessionAuthCsrf::insert(&test.session, state).await.unwrap();

            let remove_result = SessionAuthCsrf::remove(&test.session).await;
            assert!(remove_result.is_ok());

            // Attempt to get the removed token should fail
            let get_result = SessionAuthCsrf::get(&test.session).await;
            assert!(get_result.is_err());
            assert!(matches!(
                get_result,
                Err(Error::AuthError(AuthError::CsrfMissingValue))
            ));

            Ok(())
        }

        /// Tests that remove is idempotent (calling remove twice fails the second time).
        ///
        /// Verifies that after successfully removing a CSRF token, a second removal
        /// attempt fails with CsrfMissingValue error.
        ///
        /// Expected: First removal succeeds, second removal fails
        #[tokio::test]
        async fn second_removal_fails() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let _ = SessionAuthCsrf::insert(&test.session, "state")
                .await
                .unwrap();

            let first_remove = SessionAuthCsrf::remove(&test.session).await;
            assert!(first_remove.is_ok());

            let second_remove = SessionAuthCsrf::remove(&test.session).await;
            assert!(second_remove.is_err());
            assert!(matches!(
                second_remove,
                Err(Error::AuthError(AuthError::CsrfMissingValue))
            ));

            Ok(())
        }

        /// Tests error when removing non-existent CSRF token.
        ///
        /// Verifies that `remove` returns a `CsrfMissingValue` error when attempting to
        /// remove a CSRF token from an empty session.
        ///
        /// Expected: Err(Error::AuthError(AuthError::CsrfMissingValue))
        #[tokio::test]
        async fn fails_when_csrf_missing() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionAuthCsrf::remove(&test.session).await;

            // Should error due to state not being present in session
            assert!(result.is_err());
            assert!(matches!(
                result,
                Err(Error::AuthError(AuthError::CsrfMissingValue))
            ));

            Ok(())
        }
    }
}
