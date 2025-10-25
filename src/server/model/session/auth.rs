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

    /// Remove the CSRF state key from session
    pub async fn remove(session: &Session) -> Result<(), Error> {
        session
            .remove::<SessionAuthCsrf>(SESSION_AUTH_CSRF_KEY)
            .await?;

        Ok(())
    }

    /// Get & remove the CSRF state key from session
    pub async fn consume(session: &Session) -> Result<String, Error> {
        let csrf = Self::get(&session).await?;

        Self::remove(&session).await?;

        Ok(csrf)
    }
}

#[cfg(test)]
mod tests {
    use crate::server::{model::session::auth::SessionAuthCsrf, util::test::setup::test_setup};

    #[tokio::test]
    /// Test successful insertion of CSRF state
    async fn test_insert() {
        let test = test_setup().await;

        let result = SessionAuthCsrf::insert(&test.session, "string").await;

        assert!(result.is_ok())
    }

    #[tokio::test]
    /// Test successful retrieval of CSRF state
    async fn test_get() {
        let test = test_setup().await;
        let state = "string";

        let insert_result = SessionAuthCsrf::insert(&test.session, state).await;
        let get_result = SessionAuthCsrf::get(&test.session).await;

        assert!(insert_result.is_ok());
        assert!(get_result.is_ok());

        let result_state = get_result.unwrap();

        assert_eq!(result_state, state.to_string())
    }

    #[tokio::test]
    /// Test successful removal of CSRF state
    async fn test_remove() {
        let test = test_setup().await;

        let insert_result = SessionAuthCsrf::insert(&test.session, "state").await;
        let get_result = SessionAuthCsrf::get(&test.session).await;

        assert!(insert_result.is_ok());
        assert!(get_result.is_ok());

        let remove_result = SessionAuthCsrf::remove(&test.session).await;
        let get_result = SessionAuthCsrf::get(&test.session).await;

        assert!(remove_result.is_ok());

        // Should error due to state being removed from session
        assert!(get_result.is_err());
    }

    #[tokio::test]
    /// Test successful consumption of CSRF state
    async fn test_consume() {
        let test = test_setup().await;
        let state = "string";

        let insert_result = SessionAuthCsrf::insert(&test.session, state).await;
        let consume_result = SessionAuthCsrf::consume(&test.session).await;

        assert!(insert_result.is_ok());
        assert!(consume_result.is_ok());

        let result_state = consume_result.unwrap();

        assert_eq!(result_state, state.to_string());

        let get_result = SessionAuthCsrf::get(&test.session).await;

        // Should error due to state being removed from session
        assert!(get_result.is_err())
    }
}
