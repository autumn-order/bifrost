//! Tests for UserService::get_user method.
//!
//! This module verifies the user retrieval service behavior, including successful
//! retrieval of existing users, handling of nonexistent user IDs, and error handling
//! when required database tables are missing.

use bifrost::server::{error::Error, service::user::UserService};
use bifrost_test_utils::prelude::*;

/// Tests retrieving an existing user.
///
/// Verifies that the user service successfully retrieves a user record from the
/// database when provided with a valid user ID.
///
/// Expected: Ok with Some(user)
#[tokio::test]
async fn returns_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let user_service = UserService::new(&test.db);
    let result = user_service.get_user(user_model.id).await;
    assert!(result.is_ok());
    let maybe_user = result.unwrap();
    assert!(maybe_user.is_some());

    Ok(())
}

/// Tests retrieving a nonexistent user.
///
/// Verifies that the user service returns None when attempting to retrieve a
/// user with an ID that does not exist in the database.
///
/// Expected: Ok with None
#[tokio::test]
async fn returns_none_for_nonexistent_user() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let nonexistent_user_id = 1;
    let user_service = UserService::new(&test.db);
    let result = user_service.get_user(nonexistent_user_id).await;

    assert!(result.is_ok());
    let maybe_user = result.unwrap();
    assert!(maybe_user.is_none());

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the user service returns a database error when attempting to
/// retrieve a user without the required database tables being created.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let nonexistent_user_id = 1;
    let user_service = UserService::new(&test.db);
    let result = user_service.get_user(nonexistent_user_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}
