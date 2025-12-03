//! Tests for CallbackService::get_or_create_user method.
//!
//! This module verifies the user retrieval and creation logic,
//! including returning existing user IDs when provided and creating
//! new users when no user ID is specified.

use bifrost::server::{
    error::Error,
    service::{auth::callback::CallbackService, orchestrator::cache::TrackedTransaction},
};
use bifrost_test_utils::prelude::*;

/// Tests returning an existing user ID when provided.
///
/// Verifies that when a user ID is provided (Some), the method
/// simply returns that user ID without any database operations.
///
/// Expected: Ok with the same user ID
#[tokio::test]
async fn returns_existing_user_id_when_provided() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let existing_user_id = 42;
    let character_id = 1;

    let txn = TrackedTransaction::begin(&test.db).await?;

    let result =
        CallbackService::get_or_create_user(&txn, Some(existing_user_id), character_id).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), existing_user_id);

    Ok(())
}

/// Tests creating a new user when no user ID is provided.
///
/// Verifies that when no user ID is provided (None), the method
/// creates a new user with the specified character as the main character
/// and returns the new user's ID.
///
/// Expected: Ok with new user ID (should be 1 for first user)
#[tokio::test]
async fn creates_new_user_when_none_provided() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_id_eve = 123456789;
    let character_model = test
        .eve()
        .insert_mock_character(character_id_eve, 1, None, None)
        .await?;

    let txn = TrackedTransaction::begin(&test.db).await?;

    let result = CallbackService::get_or_create_user(&txn, None, character_model.id)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    let user_id = result;

    // First user created should have ID 1
    assert_eq!(user_id, 1);

    // Verify user exists in database after commit
    txn.commit().await?;

    let user_repo = bifrost::server::data::user::UserRepository::new(&test.db);
    let user_result = user_repo.get_by_id(user_id).await;
    assert!(user_result.is_ok());
    let maybe_user = user_result.unwrap();
    assert!(maybe_user.is_some());

    let (user, _) = maybe_user.unwrap();
    assert_eq!(user.id, user_id);
    assert_eq!(user.main_character_id, character_model.id);

    Ok(())
}

/// Tests creating multiple new users sequentially.
///
/// Verifies that the method can create multiple users and each
/// receives a unique, incrementing user ID.
///
/// Expected: Ok with different user IDs for each call
#[tokio::test]
async fn creates_multiple_users_with_unique_ids() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_id_1 = 111111111;
    let character_id_2 = 222222222;
    let character_id_3 = 333333333;

    let char_model_1 = test
        .eve()
        .insert_mock_character(character_id_1, 1, None, None)
        .await?;
    let char_model_2 = test
        .eve()
        .insert_mock_character(character_id_2, 2, None, None)
        .await?;
    let char_model_3 = test
        .eve()
        .insert_mock_character(character_id_3, 3, None, None)
        .await?;

    // Create first user
    let txn1 = TrackedTransaction::begin(&test.db).await?;
    let user_id_1 = CallbackService::get_or_create_user(&txn1, None, char_model_1.id)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;
    txn1.commit().await?;

    // Create second user
    let txn2 = TrackedTransaction::begin(&test.db).await?;
    let user_id_2 = CallbackService::get_or_create_user(&txn2, None, char_model_2.id)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;
    txn2.commit().await?;

    // Create third user
    let txn3 = TrackedTransaction::begin(&test.db).await?;
    let user_id_3 = CallbackService::get_or_create_user(&txn3, None, char_model_3.id)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;
    txn3.commit().await?;

    // All user IDs should be unique
    assert_eq!(user_id_1, 1);
    assert_eq!(user_id_2, 2);
    assert_eq!(user_id_3, 3);

    assert_ne!(user_id_1, user_id_2);
    assert_ne!(user_id_2, user_id_3);
    assert_ne!(user_id_1, user_id_3);

    Ok(())
}

/// Tests creating a user when transaction rolls back.
///
/// Verifies that if a user is created but the transaction is rolled back,
/// the user is not persisted to the database.
///
/// Expected: User creation succeeds but user doesn't exist after rollback
#[tokio::test]
async fn user_not_persisted_on_transaction_rollback() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_id_eve = 123456789;
    let character_model = test
        .eve()
        .insert_mock_character(character_id_eve, 1, None, None)
        .await?;

    let txn = TrackedTransaction::begin(&test.db).await?;

    let result = CallbackService::get_or_create_user(&txn, None, character_model.id).await;

    assert!(result.is_ok());
    let user_id = result.unwrap();

    // Rollback instead of commit
    drop(txn);

    // Verify user does not exist after rollback
    let user_repo = bifrost::server::data::user::UserRepository::new(&test.db);
    let user_result = user_repo.get_by_id(user_id).await;
    assert!(user_result.is_ok());
    let maybe_user = user_result.unwrap();
    assert!(maybe_user.is_none()); // User should not exist

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the method returns a database error when attempting to
/// create a user without the required database tables.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let character_id = 1;

    let txn = TrackedTransaction::begin(&test.db).await?;

    let result = CallbackService::get_or_create_user(&txn, None, character_id).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::DbErr(_)));

    Ok(())
}

/// Tests that providing Some(user_id) doesn't create a new user.
///
/// Verifies that when an existing user ID is provided, no new user
/// record is created in the database.
///
/// Expected: No new user created, original user count unchanged
#[tokio::test]
async fn does_not_create_user_when_id_provided() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create an initial user to have a baseline
    let (initial_user, _, _) = test
        .user()
        .insert_user_with_mock_character(123456789, 1, None, None)
        .await?;

    let character_id_eve = 987654321;
    let character_model = test
        .eve()
        .insert_mock_character(character_id_eve, 2, None, None)
        .await?;

    let txn = TrackedTransaction::begin(&test.db).await?;

    // Call with Some(user_id) - should not create a new user
    let result =
        CallbackService::get_or_create_user(&txn, Some(initial_user.id), character_model.id).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), initial_user.id);

    txn.commit().await?;

    // Verify only the initial user exists (no new user created)
    let user_repo = bifrost::server::data::user::UserRepository::new(&test.db);

    // User 1 should exist
    let user1_result = user_repo.get_by_id(1).await?;
    assert!(user1_result.is_some());

    // User 2 should not exist (wasn't created)
    let user2_result = user_repo.get_by_id(2).await?;
    assert!(user2_result.is_none());

    Ok(())
}
