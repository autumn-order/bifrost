//! Tests for UserCharacterService::set_main_character method.
//!
//! This module verifies the set main character service behavior, including setting
//! a new main character for a user, validating ownership, preventing unauthorized
//! changes, and transaction handling.

use bifrost::server::{error::Error, service::user::user_character::UserCharacterService};
use bifrost_test_utils::prelude::*;
use sea_orm::{EntityTrait, TransactionTrait};

/// Tests setting a different character as main.
///
/// Verifies that the service successfully updates a user's main character
/// to a different character they own.
///
/// Expected: Ok with user's main_character_id updated
#[tokio::test]
async fn sets_different_character_as_main() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user with main character
    let (user_model, _, main_char) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Add second character to user
    let (ownership2, alt_char) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 2, None, None)
        .await?;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::set_main_character(&txn, user_model.id, ownership2).await;

    assert!(result.is_ok());
    let updated_user = result.unwrap().expect("User should exist");
    assert_eq!(updated_user.main_character_id, alt_char.id);
    assert_ne!(updated_user.main_character_id, main_char.id);

    txn.commit().await?;

    // Verify in database
    let db_user = entity::prelude::BifrostUser::find_by_id(user_model.id)
        .one(&test.db)
        .await?
        .expect("User should exist");
    assert_eq!(db_user.main_character_id, alt_char.id);

    Ok(())
}

/// Tests setting same character as main (idempotent).
///
/// Verifies that setting a character as main when it's already the main
/// character completes successfully without errors.
///
/// Expected: Ok with no change
#[tokio::test]
async fn sets_same_character_as_main() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, ownership_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::set_main_character(&txn, user_model.id, ownership_model).await;

    assert!(result.is_ok());
    let updated_user = result.unwrap().expect("User should exist");
    assert_eq!(updated_user.main_character_id, character_model.id);

    txn.commit().await?;

    Ok(())
}

/// Tests error when character is owned by different user.
///
/// Verifies that the service returns an error when attempting to set
/// a character as main that belongs to a different user.
///
/// Expected: Err(AuthError::CharacterOwnedByAnotherUser)
#[tokio::test]
async fn fails_when_character_owned_by_different_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user1 with character
    let (user1, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Create user2 with different character
    let (_user2, ownership2, _) = test
        .user()
        .insert_user_with_mock_character(2, 2, None, None)
        .await?;

    let txn = test.db.begin().await?;

    // Try to set user2's character as user1's main
    let result = UserCharacterService::set_main_character(&txn, user1.id, ownership2.clone()).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        Error::AuthError(err) => {
            assert_eq!(
                format!("{:?}", err),
                format!(
                    "{:?}",
                    bifrost::server::error::auth::AuthError::CharacterOwnedByAnotherUser
                )
            );
        }
        _ => panic!("Expected AuthError::CharacterOwnedByAnotherUser"),
    }

    txn.rollback().await?;

    // Verify user1's main character unchanged
    let db_user = entity::prelude::BifrostUser::find_by_id(user1.id)
        .one(&test.db)
        .await?
        .expect("User should exist");
    assert_ne!(db_user.main_character_id, ownership2.character_id);

    Ok(())
}

/// Tests setting main character for user with multiple characters.
///
/// Verifies that the service correctly updates the main character
/// when the user owns multiple characters.
///
/// Expected: Ok with correct main character set
#[tokio::test]
async fn sets_main_character_with_multiple_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user with three characters
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (_, _) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 2, None, None)
        .await?;
    let (ownership3, char3) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 3, 3, None, None)
        .await?;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::set_main_character(&txn, user_model.id, ownership3).await;

    assert!(result.is_ok());
    let updated_user = result.unwrap().expect("User should exist");
    assert_eq!(updated_user.main_character_id, char3.id);

    txn.commit().await?;

    Ok(())
}

/// Tests transaction rollback prevents main character change.
///
/// Verifies that if the transaction is rolled back, the main character
/// change does not persist to the database.
///
/// Expected: Original main character preserved after rollback
#[tokio::test]
async fn rolls_back_on_transaction_abort() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, _, main_char) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let (ownership2, _) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 2, None, None)
        .await?;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::set_main_character(&txn, user_model.id, ownership2).await;

    assert!(result.is_ok());

    // Rollback instead of commit
    txn.rollback().await?;

    // Verify main character unchanged
    let db_user = entity::prelude::BifrostUser::find_by_id(user_model.id)
        .one(&test.db)
        .await?
        .expect("User should exist");
    assert_eq!(db_user.main_character_id, main_char.id);

    Ok(())
}

/// Tests setting main character multiple times sequentially.
///
/// Verifies that the main character can be changed multiple times
/// in sequence without issues.
///
/// Expected: Ok with final main character set correctly
#[tokio::test]
async fn changes_main_character_multiple_times() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, _, char1) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (ownership2, _char2) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 2, None, None)
        .await?;
    let (ownership3, _char3) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 3, 3, None, None)
        .await?;

    // Change to char2
    let txn1 = test.db.begin().await?;
    let result1 = UserCharacterService::set_main_character(&txn1, user_model.id, ownership2).await;
    assert!(result1.is_ok());
    txn1.commit().await?;

    // Change to char3
    let txn2 = test.db.begin().await?;
    let result2 = UserCharacterService::set_main_character(&txn2, user_model.id, ownership3).await;
    assert!(result2.is_ok());
    txn2.commit().await?;

    // Change back to char1 (using ownership from first character)
    let ownership1 = entity::prelude::BifrostUserCharacter::find_by_id(char1.id)
        .one(&test.db)
        .await?
        .expect("Ownership should exist");
    let txn3 = test.db.begin().await?;
    let result3 = UserCharacterService::set_main_character(&txn3, user_model.id, ownership1).await;
    assert!(result3.is_ok());
    txn3.commit().await?;

    // Verify final main is char1
    let db_user = entity::prelude::BifrostUser::find_by_id(user_model.id)
        .one(&test.db)
        .await?
        .expect("User should exist");
    assert_eq!(db_user.main_character_id, char1.id);

    Ok(())
}

/// Tests setting main character with different corporations.
///
/// Verifies that characters in different corporations can be set
/// as main without issues.
///
/// Expected: Ok with main character updated
#[tokio::test]
async fn sets_main_character_from_different_corporation() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // User with character in corp 1
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Add character in corp 2
    let (ownership2, char2) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 99, None, None)
        .await?;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::set_main_character(&txn, user_model.id, ownership2).await;

    assert!(result.is_ok());
    let updated_user = result.unwrap().expect("User should exist");
    assert_eq!(updated_user.main_character_id, char2.id);

    txn.commit().await?;

    Ok(())
}

/// Tests setting main character with alliance affiliation.
///
/// Verifies that characters with alliance affiliations can be set
/// as main character.
///
/// Expected: Ok with main character updated
#[tokio::test]
async fn sets_main_character_with_alliance() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Add character with alliance
    let alliance_model = test.eve().insert_mock_alliance(100, None).await?;
    let (ownership2, char2) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 2, Some(alliance_model.alliance_id), None)
        .await?;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::set_main_character(&txn, user_model.id, ownership2).await;

    assert!(result.is_ok());
    let updated_user = result.unwrap().expect("User should exist");
    assert_eq!(updated_user.main_character_id, char2.id);

    txn.commit().await?;

    Ok(())
}

/// Tests that other users are unaffected by main character change.
///
/// Verifies that changing one user's main character does not affect
/// other users in the database.
///
/// Expected: Ok with only target user modified
#[tokio::test]
async fn does_not_affect_other_users() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user1 with two characters
    let (user1, _, char1_main) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (ownership1_alt, _) = test
        .user()
        .insert_mock_character_for_user(user1.id, 2, 2, None, None)
        .await?;

    // Create user2 with character
    let (user2, _, char2_main) = test
        .user()
        .insert_user_with_mock_character(3, 3, None, None)
        .await?;

    let txn = test.db.begin().await?;

    // Change user1's main character
    let result = UserCharacterService::set_main_character(&txn, user1.id, ownership1_alt).await;

    assert!(result.is_ok());
    txn.commit().await?;

    // Verify user2 is unchanged
    let db_user2 = entity::prelude::BifrostUser::find_by_id(user2.id)
        .one(&test.db)
        .await?
        .expect("User2 should exist");
    assert_eq!(db_user2.main_character_id, char2_main.id);

    // Verify user1 changed
    let db_user1 = entity::prelude::BifrostUser::find_by_id(user1.id)
        .one(&test.db)
        .await?
        .expect("User1 should exist");
    assert_ne!(db_user1.main_character_id, char1_main.id);

    Ok(())
}

/// Tests setting main character returns updated user model.
///
/// Verifies that the service returns the updated user model with
/// the new main_character_id set.
///
/// Expected: Ok(Some(UserModel)) with updated fields
#[tokio::test]
async fn returns_updated_user_model() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let (ownership2, char2) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 2, None, None)
        .await?;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::set_main_character(&txn, user_model.id, ownership2).await;

    assert!(result.is_ok());
    let updated_user = result.unwrap();
    assert!(updated_user.is_some());

    let user = updated_user.unwrap();
    assert_eq!(user.id, user_model.id);
    assert_eq!(user.main_character_id, char2.id);

    txn.commit().await?;

    Ok(())
}

/// Tests setting main character for nonexistent user.
///
/// Verifies that attempting to set main character for a user that
/// doesn't exist returns an error due to ownership mismatch.
///
/// Expected: Err(AuthError::CharacterOwnedByAnotherUser)
#[tokio::test]
async fn returns_error_for_nonexistent_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create ownership for a character belonging to user 1
    let (_, ownership_model, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let nonexistent_user_id = 999;

    let txn = test.db.begin().await?;

    // This should fail because ownership belongs to a different user (user 1, not 999)
    let result =
        UserCharacterService::set_main_character(&txn, nonexistent_user_id, ownership_model).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        Error::AuthError(_) => {
            // Expected - ownership check happens first
        }
        _ => panic!("Expected AuthError"),
    }

    txn.rollback().await?;

    Ok(())
}

/// Tests setting main character with ownership record having same character_id.
///
/// Verifies that the ownership record's character_id is correctly used
/// when updating the user's main_character_id.
///
/// Expected: Ok with main_character_id matching ownership.character_id
#[tokio::test]
async fn sets_main_character_id_from_ownership_record() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let (ownership2, char2) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 2, None, None)
        .await?;

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::set_main_character(&txn, user_model.id, ownership2.clone()).await;

    assert!(result.is_ok());
    let updated_user = result.unwrap().expect("User should exist");

    // Verify the user's main_character_id matches the ownership's character_id
    assert_eq!(updated_user.main_character_id, ownership2.character_id);
    assert_eq!(updated_user.main_character_id, char2.id);

    txn.commit().await?;

    Ok(())
}

/// Tests preventing cross-user main character assignment.
///
/// Verifies security: user A cannot set user B's character as their main,
/// even if they pass in the correct ownership record.
///
/// Expected: Err(AuthError::CharacterOwnedByAnotherUser)
#[tokio::test]
async fn prevents_cross_user_main_character_assignment() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // User A
    let (user_a, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // User B with their character
    let (user_b, ownership_b, _) = test
        .user()
        .insert_user_with_mock_character(2, 2, None, None)
        .await?;

    let txn = test.db.begin().await?;

    // Try to set user B's character as user A's main
    let result = UserCharacterService::set_main_character(&txn, user_a.id, ownership_b).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        Error::AuthError(_) => {
            // Expected error
        }
        _ => panic!("Expected AuthError"),
    }

    txn.rollback().await?;

    // Verify user A's main unchanged
    let db_user_a = entity::prelude::BifrostUser::find_by_id(user_a.id)
        .one(&test.db)
        .await?
        .expect("User A should exist");

    // Verify user B's main unchanged
    let db_user_b = entity::prelude::BifrostUser::find_by_id(user_b.id)
        .one(&test.db)
        .await?
        .expect("User B should exist");

    assert_ne!(db_user_a.main_character_id, db_user_b.main_character_id);

    Ok(())
}

/// Tests setting main character after ownership transfer.
///
/// Verifies that after a character is transferred to a new user,
/// the new owner can set it as their main character.
///
/// Expected: Ok with new owner's main updated
#[tokio::test]
async fn sets_main_after_ownership_transfer() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // User1 with two characters
    let (user1, _, _char1) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (_, char2) = test
        .user()
        .insert_mock_character_for_user(user1.id, 2, 2, None, None)
        .await?;

    // User2
    let (user2, _, _) = test
        .user()
        .insert_user_with_mock_character(3, 3, None, None)
        .await?;

    // Transfer char2 from user1 to user2
    let txn1 = test.db.begin().await?;
    UserCharacterService::transfer_character(&txn1, char2.id, user2.id, "new_hash")
        .await
        .expect("Transfer should succeed");
    txn1.commit().await?;

    // Get updated ownership for char2
    let ownership2 = entity::prelude::BifrostUserCharacter::find_by_id(char2.id)
        .one(&test.db)
        .await?
        .expect("Ownership should exist");

    // User2 sets char2 as main
    let txn2 = test.db.begin().await?;
    let result = UserCharacterService::set_main_character(&txn2, user2.id, ownership2).await;

    assert!(result.is_ok());
    let updated_user = result.unwrap().expect("User should exist");
    assert_eq!(updated_user.main_character_id, char2.id);

    txn2.commit().await?;

    Ok(())
}
