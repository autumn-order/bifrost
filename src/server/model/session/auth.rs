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
