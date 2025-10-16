use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::server::error::Error;

pub const AUTH_LOGIN_CSRF_KEY: &str = "auth:login:csrf";

#[derive(Default, Deserialize, Serialize, Debug)]
pub struct AuthLoginCsrf(pub String);

impl AuthLoginCsrf {
    // Insert CSRF state key into session
    pub async fn insert(session: &Session, state: String) -> Result<(), Error> {
        session
            .insert(AUTH_LOGIN_CSRF_KEY, AuthLoginCsrf(state))
            .await?;

        Ok(())
    }

    // Get the CSRF state key from session
    pub async fn get(session: &Session) -> Result<String, Error> {
        match session.get(AUTH_LOGIN_CSRF_KEY).await? {
            Some(csrf) => Ok(csrf),
            None => Err(Error::AuthCsrfEmptySession),
        }
    }

    // Remove the CSRF state key from session
    pub async fn remove(session: &Session) -> Result<(), Error> {
        session.remove::<AuthLoginCsrf>(AUTH_LOGIN_CSRF_KEY).await?;

        Ok(())
    }

    // Get & remove the CSRF state key from session
    pub async fn consume(session: &Session) -> Result<String, Error> {
        let csrf = Self::get(&session).await?;

        Self::remove(&session).await?;

        Ok(csrf)
    }
}
