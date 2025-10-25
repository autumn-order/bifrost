use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::server::error::Error;

pub const SESSION_USER_ID_KEY: &str = "bifrost:user:id";

#[derive(Default, Deserialize, Serialize, Debug)]
pub struct SessionUserId(pub String);

impl SessionUserId {
    /// Insert user ID into session
    pub async fn insert(session: &Session, user_id: i32) -> Result<(), Error> {
        session
            .insert(SESSION_USER_ID_KEY, SessionUserId(user_id.to_string()))
            .await?;

        Ok(())
    }

    /// Get user ID from session
    pub async fn get(session: &Session) -> Result<Option<i32>, Error> {
        session
            .get::<SessionUserId>(SESSION_USER_ID_KEY)
            .await?
            .map(|SessionUserId(id_str)| {
                id_str.parse::<i32>().map_err(|e| {
                    Error::ParseError(format!("Failed to parse session user id: {}", e))
                })
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    mod session_insert_user_id_tests {
        use crate::server::{
            error::Error, model::session::user::SessionUserId, util::test::setup::test_setup,
        };

        #[tokio::test]
        /// Expect success when inserting valid user ID into session
        async fn test_insert_session_user_id_success() -> Result<(), Error> {
            let test = test_setup().await;

            let user_id = 1;
            let result = SessionUserId::insert(&test.session, user_id).await;

            assert!(result.is_ok());

            Ok(())
        }
    }

    mod session_get_user_id_tests {
        use crate::server::{
            error::Error,
            model::session::user::{SessionUserId, SESSION_USER_ID_KEY},
            util::test::setup::test_setup,
        };

        #[tokio::test]
        /// Expect Some when user ID is present in session
        async fn test_get_session_user_id_some() -> Result<(), Error> {
            let test = test_setup().await;

            let user_id = 1;
            let _ = SessionUserId::insert(&test.session, user_id).await?;

            let result = SessionUserId::get(&test.session).await;

            assert!(result.is_ok());
            let user_id_opt = result.unwrap();

            assert!(user_id_opt.is_some());
            let session_user_id = user_id_opt.unwrap();

            assert_eq!(session_user_id, user_id);

            Ok(())
        }

        #[tokio::test]
        /// Expect None when no user ID is present in session
        async fn test_get_session_user_id_none() -> Result<(), Error> {
            let test = test_setup().await;

            let result = SessionUserId::get(&test.session).await;

            assert!(result.is_ok());
            let user_id_opt = result.unwrap();

            assert!(user_id_opt.is_none());

            Ok(())
        }

        #[tokio::test]
        /// Expect parse error when user ID inserted into session is not an i32
        async fn test_get_session_user_id_parse_error() -> Result<(), Error> {
            let test = test_setup().await;

            // Insert a user ID string which will fail i32 parse
            let user_id = "invalid_id";
            test.session
                .insert(SESSION_USER_ID_KEY, SessionUserId(user_id.to_string()))
                .await?;

            let result = SessionUserId::get(&test.session).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
