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
    use super::*;

    mod insert {
        use super::*;
        use bifrost_test_utils::prelude::*;

        /// Tests successful insertion of user ID into session.
        ///
        /// Verifies that a valid user ID can be stored in the session without errors.
        ///
        /// Expected: Ok(())
        #[tokio::test]
        async fn inserts_user_id_into_session() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let user_id = 1;
            let result = SessionUserId::insert(&test.session, user_id).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Tests that inserted user ID can be retrieved with correct value.
        ///
        /// Verifies that the user ID stored in the session matches the original value
        /// when retrieved, ensuring data integrity during storage.
        ///
        /// Expected: Ok(()) and retrieved value matches inserted value
        #[tokio::test]
        async fn inserted_user_id_is_retrievable() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let user_id = 12345;

            let insert_result = SessionUserId::insert(&test.session, user_id).await;
            assert!(insert_result.is_ok());

            let get_result = SessionUserId::get(&test.session).await;
            assert!(get_result.is_ok());
            assert_eq!(get_result.unwrap(), Some(user_id));

            Ok(())
        }

        /// Tests that inserting a new user ID overwrites the previous one.
        ///
        /// Verifies that when a user ID is inserted multiple times, the latest value
        /// overwrites the previous one, ensuring only the most recent ID is stored.
        ///
        /// Expected: Ok(()) and retrieved value matches the latest inserted value
        #[tokio::test]
        async fn overwrites_existing_user_id() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let first_user_id = 100;
            let second_user_id = 200;

            let _ = SessionUserId::insert(&test.session, first_user_id)
                .await
                .unwrap();
            let _ = SessionUserId::insert(&test.session, second_user_id)
                .await
                .unwrap();

            let result = SessionUserId::get(&test.session).await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), Some(second_user_id));

            Ok(())
        }

        /// Tests insertion of zero as user ID.
        ///
        /// Verifies that zero can be stored as a user ID (edge case),
        /// though typically user IDs start from 1.
        ///
        /// Expected: Ok(())
        #[tokio::test]
        async fn inserts_zero_user_id() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionUserId::insert(&test.session, 0).await;

            assert!(result.is_ok());

            let get_result = SessionUserId::get(&test.session).await;
            assert!(get_result.is_ok());
            assert_eq!(get_result.unwrap(), Some(0));

            Ok(())
        }

        /// Tests insertion of negative user ID.
        ///
        /// Verifies that negative numbers can be stored (edge case),
        /// though typically user IDs are positive.
        ///
        /// Expected: Ok(())
        #[tokio::test]
        async fn inserts_negative_user_id() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionUserId::insert(&test.session, -1).await;

            assert!(result.is_ok());

            let get_result = SessionUserId::get(&test.session).await;
            assert!(get_result.is_ok());
            assert_eq!(get_result.unwrap(), Some(-1));

            Ok(())
        }

        /// Tests insertion of large user ID.
        ///
        /// Verifies that large i32 values can be stored and retrieved correctly.
        ///
        /// Expected: Ok(())
        #[tokio::test]
        async fn inserts_large_user_id() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let large_id = i32::MAX;

            let result = SessionUserId::insert(&test.session, large_id).await;

            assert!(result.is_ok());

            let get_result = SessionUserId::get(&test.session).await;
            assert!(get_result.is_ok());
            assert_eq!(get_result.unwrap(), Some(large_id));

            Ok(())
        }
    }

    mod get {
        use super::*;
        use bifrost_test_utils::prelude::*;

        /// Tests successful retrieval of user ID from session.
        ///
        /// Verifies that a stored user ID can be retrieved and parsed correctly,
        /// returning `Some(user_id)`.
        ///
        /// Expected: Ok(Some(1))
        #[tokio::test]
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

        /// Tests retrieval when no user ID is present in session.
        ///
        /// Verifies that `get` returns `Ok(None)` when the session does not contain
        /// a user ID, indicating an unauthenticated request.
        ///
        /// Expected: Ok(None)
        #[tokio::test]
        async fn returns_none_when_user_id_missing() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let result = SessionUserId::get(&test.session).await;

            assert!(result.is_ok());
            let user_id_opt = result.unwrap();

            assert!(user_id_opt.is_none());

            Ok(())
        }

        /// Tests parse error when user ID has invalid format.
        ///
        /// Verifies that `get` returns a parse error when the session contains a user ID
        /// that cannot be parsed as an i32, detecting session data corruption.
        ///
        /// Expected: Err(Error::ParseError)
        #[tokio::test]
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

        /// Tests parse error when user ID is a float.
        ///
        /// Verifies that `get` returns a parse error when the session contains a
        /// floating point number that cannot be parsed as an i32.
        ///
        /// Expected: Err(Error::ParseError)
        #[tokio::test]
        async fn fails_for_float_user_id() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            // Insert a float value that will fail i32 parse
            test.session
                .insert(SESSION_USER_ID_KEY, SessionUserId("123.45".to_string()))
                .await?;

            let result = SessionUserId::get(&test.session).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::ParseError(_))));

            Ok(())
        }

        /// Tests parse error when user ID is empty string.
        ///
        /// Verifies that `get` returns a parse error when the session contains an
        /// empty string for the user ID.
        ///
        /// Expected: Err(Error::ParseError)
        #[tokio::test]
        async fn fails_for_empty_string_user_id() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            // Insert empty string that will fail i32 parse
            test.session
                .insert(SESSION_USER_ID_KEY, SessionUserId("".to_string()))
                .await?;

            let result = SessionUserId::get(&test.session).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::ParseError(_))));

            Ok(())
        }

        /// Tests parse error when user ID exceeds i32 range.
        ///
        /// Verifies that `get` returns a parse error when the session contains a
        /// number that is too large to fit in an i32.
        ///
        /// Expected: Err(Error::ParseError)
        #[tokio::test]
        async fn fails_for_out_of_range_user_id() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            // Insert a value larger than i32::MAX
            test.session
                .insert(
                    SESSION_USER_ID_KEY,
                    SessionUserId("9999999999999999999".to_string()),
                )
                .await?;

            let result = SessionUserId::get(&test.session).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::ParseError(_))));

            Ok(())
        }

        /// Tests that multiple retrievals return the same value.
        ///
        /// Verifies that `get` can be called multiple times on the same session
        /// and returns the same value (non-destructive read).
        ///
        /// Expected: Ok(Some(user_id)) for all calls
        #[tokio::test]
        async fn multiple_gets_return_same_value() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;
            let user_id = 42;
            let _ = SessionUserId::insert(&test.session, user_id).await.unwrap();

            let first_get = SessionUserId::get(&test.session).await;
            assert!(first_get.is_ok());
            assert_eq!(first_get.unwrap(), Some(user_id));

            let second_get = SessionUserId::get(&test.session).await;
            assert!(second_get.is_ok());
            assert_eq!(second_get.unwrap(), Some(user_id));

            let third_get = SessionUserId::get(&test.session).await;
            assert!(third_get.is_ok());
            assert_eq!(third_get.unwrap(), Some(user_id));

            Ok(())
        }
    }
}
