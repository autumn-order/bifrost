//! User session data models.
//!
//! This module provides type-safe wrappers for storing and retrieving user identity
//! information in the session. The user ID is stored as a string and validated during
//! retrieval to ensure type safety and detect session data corruption.

use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::server::error::Error;

/// Session key for storing user ID.
///
/// This constant defines the Redis key used to store the authenticated user's ID in the
/// session. The key is namespaced under "bifrost:user:" to avoid collisions with other
/// session data.
pub const SESSION_USER_ID_KEY: &str = "bifrost:user:id";

/// Session wrapper for user ID storage.
///
/// This struct wraps the user ID as a string for serialization to the session store.
/// The string representation allows for flexible storage while maintaining type safety
/// through parsing on retrieval. The wrapper implements `Default` for initialization
/// and `Debug` for diagnostics.
#[derive(Default, Deserialize, Serialize, Debug)]
pub struct SessionUserId(pub String);

impl SessionUserId {
    /// Inserts the user ID into the session.
    ///
    /// Stores the authenticated user's ID in the session, converting it to a string for
    /// serialization. This is typically called after successful OAuth callback to establish
    /// a user session. The user ID is stored under the `SESSION_USER_ID_KEY` and persisted
    /// to the Redis session store.
    ///
    /// # Arguments
    /// - `session` - User's session for storing the ID
    /// - `user_id` - Database user ID to store (converted to string)
    ///
    /// # Returns
    /// - `Ok(())` - User ID successfully stored in session
    /// - `Err(Error)` - Session storage failed (Redis error, serialization error)
    pub async fn insert(session: &Session, user_id: i32) -> Result<(), Error> {
        session
            .insert(SESSION_USER_ID_KEY, SessionUserId(user_id.to_string()))
            .await?;

        Ok(())
    }

    /// Retrieves the user ID from the session.
    ///
    /// Fetches the user ID from the session store and parses it from string to i32. Returns
    /// `None` if no user ID is present in the session (unauthenticated request), or an error
    /// if the stored value cannot be parsed as an integer (indicating session data corruption).
    /// This method is used by protected endpoints to verify user authentication.
    ///
    /// # Arguments
    /// - `session` - User's session to retrieve the ID from
    ///
    /// # Returns
    /// - `Ok(Some(user_id))` - User ID found and successfully parsed
    /// - `Ok(None)` - No user ID present in session (not authenticated)
    /// - `Err(Error::ParseError)` - User ID present but cannot be parsed as i32
    /// - `Err(Error)` - Session retrieval failed (Redis error)
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
    mod insert {
        use bifrost_test_utils::prelude::*;

        use crate::server::model::session::user::SessionUserId;

        #[tokio::test]
        /// Tests successful insertion of user ID into session.
        ///
        /// Verifies that a valid user ID can be stored in the session without errors.
        ///
        /// Expected: Ok(())
        async fn inserts_user_id_into_session() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let user_id = 1;
            let result = SessionUserId::insert(&test.session, user_id).await;

            assert!(result.is_ok());

            Ok(())
        }
    }

    mod get {
        use bifrost_test_utils::prelude::*;

        use crate::server::model::session::user::{SessionUserId, SESSION_USER_ID_KEY};

        #[tokio::test]
        /// Tests successful retrieval of user ID from session.
        ///
        /// Verifies that a stored user ID can be retrieved and parsed correctly,
        /// returning `Some(user_id)`.
        ///
        /// Expected: Ok(Some(1))
        async fn retrieves_user_id_from_session() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let user_id = 1;
            let _ = SessionUserId::insert(&test.session, user_id).await.unwrap();

            let result = SessionUserId::get(&test.session).await;

            assert!(result.is_ok());
            let user_id_opt = result.unwrap();

            assert!(user_id_opt.is_some());
            let session_user_id = user_id_opt.unwrap();

            assert_eq!(session_user_id, user_id);

            Ok(())
        }

        #[tokio::test]
        /// Tests retrieval when no user ID is present in session.
        ///
        /// Verifies that `get` returns `Ok(None)` when the session does not contain
        /// a user ID, indicating an unauthenticated request.
        ///
        /// Expected: Ok(None)
        async fn returns_none_when_user_id_missing() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionUserId::get(&test.session).await;

            assert!(result.is_ok());
            let user_id_opt = result.unwrap();

            assert!(user_id_opt.is_none());

            Ok(())
        }

        #[tokio::test]
        /// Tests parse error when user ID has invalid format.
        ///
        /// Verifies that `get` returns a parse error when the session contains a user ID
        /// that cannot be parsed as an i32, detecting session data corruption.
        ///
        /// Expected: Err(Error::ParseError)
        async fn fails_for_invalid_user_id_format() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

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
