use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::server::error::Error;

pub const SESSION_AUTH_CSRF_KEY: &str = "bifrost:auth:csrf";

#[derive(Default, Deserialize, Serialize, Debug)]
pub struct SessionAuthCsrf(pub String);

impl SessionAuthCsrf {
    /// Insert CSRF state into session
    pub async fn insert(session: &Session, state: &str) -> Result<(), Error> {
        session
            .insert(SESSION_AUTH_CSRF_KEY, SessionAuthCsrf(state.to_string()))
            .await?;

        Ok(())
    }

    /// Get the CSRF state from session
    pub async fn get(session: &Session) -> Result<String, Error> {
        match session.get(SESSION_AUTH_CSRF_KEY).await? {
            Some(csrf) => Ok(csrf),
            None => Err(Error::AuthCsrfEmptySession),
        }
    }

    /// Remove the CSRF state key from session & return it
    pub async fn remove(session: &Session) -> Result<Option<String>, Error> {
        match session.remove(SESSION_AUTH_CSRF_KEY).await? {
            Some(csrf) => Ok(csrf),
            None => Err(Error::AuthCsrfEmptySession),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::server::{model::session::auth::SessionAuthCsrf, util::test::setup::test_setup};

    #[tokio::test]
    /// Expect success when inserting CSRF into session
    async fn test_session_insert_csrf_succcess() {
        let test = test_setup().await;

        let result = SessionAuthCsrf::insert(&test.session, "string").await;

        assert!(result.is_ok())
    }

    mod session_csrf_get_tests {
        use crate::server::{
            error::Error, model::session::auth::SessionAuthCsrf, util::test::setup::test_setup,
        };

        /// Expect success when retrieving CSRF from session
        #[tokio::test]
        async fn test_session_get_csrf_success() -> Result<(), Error> {
            let test = test_setup().await;
            let state = "string";

            let _ = SessionAuthCsrf::insert(&test.session, state).await?;
            let result = SessionAuthCsrf::get(&test.session).await;

            assert!(result.is_ok());
            let result_state = result.unwrap();

            assert_eq!(result_state, state.to_string());

            Ok(())
        }

        /// Expect Error when no state is present in session
        #[tokio::test]
        async fn test_session_get_csrf_error() -> Result<(), Error> {
            let test = test_setup().await;

            let result = SessionAuthCsrf::get(&test.session).await;

            // Should error due to state not being present in session
            assert!(result.is_err());
            assert!(matches!(result, Err(Error::AuthCsrfEmptySession)));

            Ok(())
        }
    }

    mod session_csrf_remove_tests {
        use crate::server::{
            error::Error, model::session::auth::SessionAuthCsrf, util::test::setup::test_setup,
        };

        /// Expect successful removal of CSRF state from session
        #[tokio::test]
        async fn test_remove_session_csrf_success() -> Result<(), Error> {
            let test = test_setup().await;

            let _ = SessionAuthCsrf::insert(&test.session, "state").await?;

            let result = SessionAuthCsrf::remove(&test.session).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Error when no state is present in session
        #[tokio::test]
        async fn test_remove_session_csrf_error() -> Result<(), Error> {
            let test = test_setup().await;

            let result = SessionAuthCsrf::remove(&test.session).await;

            // Should error due to state not being present in session
            assert!(result.is_err());
            assert!(matches!(result, Err(Error::AuthCsrfEmptySession)));

            Ok(())
        }
    }
}
