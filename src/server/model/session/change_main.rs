use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::server::error::Error;

pub const SESSION_USER_CHANGE_MAIN_KEY: &str = "bifrost:user:change_main";

#[derive(Default, Deserialize, Serialize, Debug)]
pub struct SessionUserChangeMain(pub bool);

impl SessionUserChangeMain {
    /// Insert change main flag into session
    pub async fn insert(session: &Session, change_main: bool) -> Result<(), Error> {
        session
            .insert(
                SESSION_USER_CHANGE_MAIN_KEY,
                SessionUserChangeMain(change_main),
            )
            .await?;

        Ok(())
    }

    /// Remove & return value of change main flag from session
    pub async fn remove(session: &Session) -> Result<Option<bool>, Error> {
        Ok(session.remove(SESSION_USER_CHANGE_MAIN_KEY).await?)
    }
}
