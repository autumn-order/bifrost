//! CSRF token validation utilities for authentication flows.
//!
//! This module provides functions to validate Cross-Site Request Forgery (CSRF) tokens
//! during OAuth authentication flows. CSRF tokens are stored in the session during login
//! initiation and validated during the OAuth callback to prevent CSRF attacks.

use tower_sessions::Session;

use crate::server::{
    error::{auth::AuthError, Error},
    model::session::auth::SessionAuthCsrf,
};

/// Validates CSRF state token from OAuth callback against the session value.
///
/// Retrieves and removes the CSRF state token from the session, then compares it with
/// the state token provided in the OAuth callback URL. This prevents CSRF attacks by
/// ensuring the callback originated from a legitimate login request initiated by this
/// application. The token is removed from the session after validation (whether successful
/// or not) to prevent reuse.
///
/// # Arguments
/// - `session` - User's session containing the stored CSRF state token
/// - `csrf_state` - CSRF state token from the OAuth callback URL parameters
///
/// # Returns
/// - `Ok(())` - CSRF state is valid (matches the session value)
/// - `Err(Error::AuthError(AuthError::CsrfValidationFailed))` - State mismatch or not found in session
/// - `Err(Error)` - Session retrieval error
pub async fn validate_csrf(session: &Session, csrf_state: &str) -> Result<(), Error> {
    let stored_state = SessionAuthCsrf::remove(session).await?;

    if let Some(state) = stored_state {
        if state == csrf_state {
            return Ok(());
        }
    }

    Err(Error::AuthError(AuthError::CsrfValidationFailed))
}

#[cfg(test)]
pub mod tests {
    use axum::{http::StatusCode, response::IntoResponse};
    use bifrost_test_utils::prelude::*;

    use crate::server::{
        controller::util::csrf::validate_csrf, model::session::auth::SessionAuthCsrf,
    };

    #[tokio::test]
    /// Tests successful validation of CSRF state.
    ///
    /// Verifies that `validate_csrf` returns `Ok(())` when the CSRF state in the session
    /// matches the provided state parameter.
    ///
    /// Expected: 200 success
    async fn validates_csrf_successfully() -> Result<(), TestError> {
        let test = test_setup_with_tables!()?;
        let state = "state";

        let _ = SessionAuthCsrf::insert(&test.session, state).await.unwrap();
        let result = validate_csrf(&test.session, state).await;

        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    /// Tests failed validation of CSRF state due to mismatch.
    ///
    /// Verifies that `validate_csrf` returns an error when the CSRF state in the session
    /// does not match the provided state parameter, returning a 400 Bad Request response.
    ///
    /// Expected: 400 bad request
    async fn fails_for_csrf_mismatch() -> Result<(), TestError> {
        let test = test_setup_with_tables!()?;
        let state = "state";

        let _ = SessionAuthCsrf::insert(&test.session, "different_state")
            .await
            .unwrap();
        let result = validate_csrf(&test.session, state).await;

        assert!(result.is_err());
        let resp = result.unwrap_err().into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        Ok(())
    }

    #[tokio::test]
    /// Tests failed validation of CSRF state when not present in session.
    ///
    /// Verifies that `validate_csrf` returns an error when no CSRF state exists in the
    /// session, returning a 500 Internal Server Error response.
    ///
    /// Expected: 500 internal server error
    async fn fails_when_csrf_not_in_session() -> Result<(), TestError> {
        let test = test_setup_with_tables!()?;
        let state = "state";

        // Attempt to validate result despite no state being inserted into sesison
        let result = validate_csrf(&test.session, state).await;

        assert!(result.is_err());
        let resp = result.unwrap_err().into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        Ok(())
    }
}
