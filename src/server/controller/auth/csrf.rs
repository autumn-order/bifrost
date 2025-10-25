use tower_sessions::Session;

use crate::server::{error::Error, model::session::auth::SessionAuthCsrf};

/// Validate that the session CSRF state exists and matches `state`.
/// Returns `Ok(())` when valid or the appropriate `Error` otherwise.
pub async fn validate_csrf(session: &Session, csrf_state: &str) -> Result<(), Error> {
    let stored_state = SessionAuthCsrf::remove(session).await?;

    if let Some(state) = stored_state {
        if state == csrf_state {
            return Ok(());
        }
    }

    Err(Error::AuthCsrfInvalidState)
}

#[cfg(test)]
pub mod tests {
    use axum::{http::StatusCode, response::IntoResponse};

    use crate::server::{
        controller::auth::csrf::validate_csrf, model::session::auth::SessionAuthCsrf,
        util::test::setup::test_setup,
    };

    #[tokio::test]
    /// Tests successful validation of CSRF state
    ///
    /// 200 success
    async fn test_validate_csrf_success() {
        let test = test_setup().await;
        let state = "state";

        let insert_result = SessionAuthCsrf::insert(&test.session, state).await;
        let validate_result = validate_csrf(&test.session, state).await;

        assert!(insert_result.is_ok());
        assert!(validate_result.is_ok())
    }

    #[tokio::test]
    /// Tests failed validation of CSRF state due to mismatch
    ///
    /// 400 bad request
    async fn test_validate_csrf_mismatch() {
        let test = test_setup().await;
        let state = "state";

        let insert_result = SessionAuthCsrf::insert(&test.session, "different_state").await;
        let validate_result = validate_csrf(&test.session, state).await;

        assert!(insert_result.is_ok());
        assert!(validate_result.is_err());

        let resp = validate_result.unwrap_err().into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    /// Tests failed validation of CSRF state due to session error
    ///
    /// 500 internal server error
    async fn test_validate_csrf_session_error() {
        let test = test_setup().await;
        let state = "state";

        // Attempt to validate result despite no state being inserted into sesison
        let validate_result = validate_csrf(&test.session, state).await;

        assert!(validate_result.is_err());

        let resp = validate_result.unwrap_err().into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
