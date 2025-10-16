use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::server::error::Error;

pub const AUTH_LOGIN_CSRF_KEY: &str = "auth:login:csrf";

#[derive(Default, Deserialize, Serialize, Debug)]
pub struct AuthLoginCsrf(pub String);

impl AuthLoginCsrf {
    /// Insert CSRF state key into session
    pub async fn insert(session: &Session, state: &str) -> Result<(), Error> {
        session
            .insert(AUTH_LOGIN_CSRF_KEY, AuthLoginCsrf(state.to_string()))
            .await?;

        Ok(())
    }

    /// Get the CSRF state key from session
    pub async fn get(session: &Session) -> Result<String, Error> {
        match session.get(AUTH_LOGIN_CSRF_KEY).await? {
            Some(csrf) => Ok(csrf),
            None => Err(Error::AuthCsrfEmptySession),
        }
    }

    /// Remove the CSRF state key from session
    pub async fn remove(session: &Session) -> Result<(), Error> {
        session.remove::<AuthLoginCsrf>(AUTH_LOGIN_CSRF_KEY).await?;

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
    use std::sync::Arc;

    use tower_sessions::{MemoryStore, Session};

    use crate::server::model::session::AuthLoginCsrf;

    fn setup() -> Session {
        let store = Arc::new(MemoryStore::default());
        Session::new(None, store, None)
    }

    #[tokio::test]
    /// Test successful insertion of CSRF state
    async fn test_insert() {
        let session = setup();

        let result = AuthLoginCsrf::insert(&session, "string").await;

        assert!(result.is_ok())
    }

    #[tokio::test]
    /// Test successful retrieval of CSRF state
    async fn test_get() {
        let session = setup();
        let state = "string";

        let insert_result = AuthLoginCsrf::insert(&session, state).await;
        let get_result = AuthLoginCsrf::get(&session).await;

        assert!(insert_result.is_ok());
        assert!(get_result.is_ok());

        let result_state = get_result.unwrap();

        assert_eq!(result_state, state.to_string())
    }

    #[tokio::test]
    /// Test successful removal of CSRF state
    async fn test_remove() {
        let session = setup();

        let insert_result = AuthLoginCsrf::insert(&session, "state").await;
        let get_result = AuthLoginCsrf::get(&session).await;

        assert!(insert_result.is_ok());
        assert!(get_result.is_ok());

        let remove_result = AuthLoginCsrf::remove(&session).await;
        let get_result = AuthLoginCsrf::get(&session).await;

        assert!(remove_result.is_ok());

        // Should error due to state being removed from session
        assert!(get_result.is_err());
    }

    #[tokio::test]
    /// Test successful consumption of CSRF state
    async fn test_consume() {
        let session = setup();
        let state = "string";

        let insert_result = AuthLoginCsrf::insert(&session, state).await;
        let consume_result = AuthLoginCsrf::consume(&session).await;

        assert!(insert_result.is_ok());
        assert!(consume_result.is_ok());

        let result_state = consume_result.unwrap();

        assert_eq!(result_state, state.to_string());

        let get_result = AuthLoginCsrf::get(&session).await;

        // Should error due to state being removed from session
        assert!(get_result.is_err())
    }
}
