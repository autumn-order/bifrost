use tower_sessions::Session;

use crate::server::{
    error::{auth::AuthError, Error},
    model::session::auth::SessionAuthCsrf,
};

/// Validate that the session CSRF state exists and matches `state`.
/// Returns `Ok(())` when valid or the appropriate `Error` otherwise.
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
    /// Tests successful validation of CSRF state
    ///
    /// 200 success
    async fn validates_csrf_successfully() -> Result<(), TestError> {
        let test = test_setup_with_tables!()?;
        let state = "state";

        let _ = SessionAuthCsrf::insert(&test.session, state).await.unwrap();
        let result = validate_csrf(&test.session, state).await;

        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    /// Tests failed validation of CSRF state due to mismatch
    ///
    /// 400 bad request
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
    /// Tests failed validation of CSRF state due to session error
    ///
    /// 500 internal server error
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
