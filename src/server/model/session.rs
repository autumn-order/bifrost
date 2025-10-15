use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::server::error::Error;

pub const AUTH_LOGIN_CSRF_KEY: &str = "auth:login:csrf";

#[derive(Default, Deserialize, Serialize, Debug)]
pub struct AuthLoginCsrf(pub String);

impl AuthLoginCsrf {
    pub async fn get(session: &Session) -> Result<Option<String>, Error> {
        let csrf = session.get(AUTH_LOGIN_CSRF_KEY).await?;

        Ok(csrf)
    }

    pub async fn insert(session: &Session, state: String) -> Result<(), Error> {
        session
            .insert(AUTH_LOGIN_CSRF_KEY, AuthLoginCsrf(state))
            .await?;

        Ok(())
    }
}
