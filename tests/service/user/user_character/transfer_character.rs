//! Tests for UserCharacterService::transfer_character method.
//!
//! This module verifies the character transfer service behavior, including transferring
//! ownership between users, handling main character updates, user cleanup when no
//! characters remain, and error handling for missing ownership or users.

use bifrost::server::{error::Error, service::user::user_character::UserCharacterService};
use bifrost_test_utils::prelude::*;
use sea_orm::{EntityTrait, TransactionTrait};

/// Tests transferring a character from one user to another.
///
/// Verifies that the service successfully transfers ownership of a character
/// from the original user to a new user, updating the ownership record.
///
/// Expected: Ok with ownership transferred to new user
#[tokio::test]
async fn transfers_character_between_users() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user1 with character
    let (user1, _, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Create user2
    let char2 = test.eve().insert_mock_character(2, 2, None, None).await?;
    let user2 = test.user().insert_user(char2.id).await?;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::transfer_character(
        &txn,
        character_model.id,
        user2.id,
        "new_owner_hash",
    )
    .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.character_id, character_model.id);
    assert_eq!(ownership.user_id, user2.id);
    assert_ne!(ownership.user_id, user1.id);

    txn.commit().await?;

    // Verify ownership in database
    let db_ownership = entity::prelude::BifrostUserCharacter::find_by_id(character_model.id)
        .one(&test.db)
        .await?
        .expect("Ownership should exist");
    assert_eq!(db_ownership.user_id, user2.id);

    Ok(())
}

/// Tests transferring non-main character.
///
/// Verifies that transferring a character that is not the user's main character
/// completes successfully without affecting the user or their main character.
///
/// Expected: Ok with character transferred, original user unchanged
#[tokio::test]
async fn transfers_non_main_character() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user1 with main character
    let (user1, _, main_char) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Add second character to user1 (not main)
    let (_, alt_char) = test
        .user()
        .insert_mock_character_for_user(user1.id, 2, 2, None, None)
        .await?;

    // Create user2
    let char3 = test.eve().insert_mock_character(3, 3, None, None).await?;
    let user2 = test.user().insert_user(char3.id).await?;

    let txn = test.db.begin().await?;

    // Transfer alt character (not main) to user2
    let result =
        UserCharacterService::transfer_character(&txn, alt_char.id, user2.id, "new_owner_hash")
            .await;

    assert!(result.is_ok());
    txn.commit().await?;

    // Verify user1 still exists with same main character
    let user1_check = entity::prelude::BifrostUser::find_by_id(user1.id)
        .one(&test.db)
        .await?
        .expect("User1 should still exist");
    assert_eq!(user1_check.main_character_id, main_char.id);

    // Verify alt character now belongs to user2
    let ownership = entity::prelude::BifrostUserCharacter::find_by_id(alt_char.id)
        .one(&test.db)
        .await?
        .expect("Ownership should exist");
    assert_eq!(ownership.user_id, user2.id);

    Ok(())
}

/// Tests transferring main character when user has other characters.
///
/// Verifies that when transferring a user's main character and they have
/// other characters, the service automatically selects a new main character.
///
/// Expected: Ok with transfer complete and new main character assigned
#[tokio::test]
async fn updates_main_character_when_transferring_main_with_other_chars() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user1 with main character
    let (user1, _, main_char) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Add second character to user1
    let (_, alt_char) = test
        .user()
        .insert_mock_character_for_user(user1.id, 2, 2, None, None)
        .await?;

    // Create user2
    let char3 = test.eve().insert_mock_character(3, 3, None, None).await?;
    let user2 = test.user().insert_user(char3.id).await?;

    let txn = test.db.begin().await?;

    // Transfer main character to user2
    let result =
        UserCharacterService::transfer_character(&txn, main_char.id, user2.id, "new_owner_hash")
            .await;

    assert!(result.is_ok());
    txn.commit().await?;

    // Verify user1 still exists but with different main character
    let user1_check = entity::prelude::BifrostUser::find_by_id(user1.id)
        .one(&test.db)
        .await?
        .expect("User1 should still exist");
    assert_ne!(user1_check.main_character_id, main_char.id);
    assert_eq!(user1_check.main_character_id, alt_char.id);

    Ok(())
}

/// Tests deleting user when transferring their only character.
///
/// Verifies that when a user's only character (which must be their main)
/// is transferred to another user, the original user is deleted.
///
/// Expected: Ok with transfer complete and original user deleted
#[tokio::test]
async fn deletes_user_when_transferring_only_character() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user1 with only one character
    let (user1, _, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Create user2
    let char2 = test.eve().insert_mock_character(2, 2, None, None).await?;
    let user2 = test.user().insert_user(char2.id).await?;

    let txn = test.db.begin().await?;

    // Transfer user1's only character to user2
    let result = UserCharacterService::transfer_character(
        &txn,
        character_model.id,
        user2.id,
        "new_owner_hash",
    )
    .await;

    assert!(result.is_ok());
    txn.commit().await?;

    // Verify user1 no longer exists
    let user1_check = entity::prelude::BifrostUser::find_by_id(user1.id)
        .one(&test.db)
        .await?;
    assert!(user1_check.is_none(), "User1 should be deleted");

    // Verify character now belongs to user2
    let ownership = entity::prelude::BifrostUserCharacter::find_by_id(character_model.id)
        .one(&test.db)
        .await?
        .expect("Ownership should exist");
    assert_eq!(ownership.user_id, user2.id);

    Ok(())
}

/// Tests transferring character to same user.
///
/// Verifies that transferring a character to the user who already owns it
/// simply updates the owner hash without other changes.
///
/// Expected: Ok with owner hash updated, no structural changes
#[tokio::test]
async fn transfers_character_to_same_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, original_ownership, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let original_owner_hash = original_ownership.owner_hash.clone();
    let new_owner_hash = "refreshed_owner_hash";

    let txn = test.db.begin().await?;

    let result = UserCharacterService::transfer_character(
        &txn,
        character_model.id,
        user_model.id,
        new_owner_hash,
    )
    .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.user_id, user_model.id);
    assert_eq!(ownership.owner_hash, new_owner_hash);
    assert_ne!(ownership.owner_hash, original_owner_hash);

    txn.commit().await?;

    // Verify user still exists
    let user_check = entity::prelude::BifrostUser::find_by_id(user_model.id)
        .one(&test.db)
        .await?;
    assert!(user_check.is_some());

    Ok(())
}

/// Tests updating owner hash during transfer.
///
/// Verifies that the owner hash is correctly updated during the transfer
/// process to reflect the new ownership verification token.
///
/// Expected: Ok with new owner hash set
#[tokio::test]
async fn updates_owner_hash() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (_user1, original_ownership, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let char2 = test.eve().insert_mock_character(2, 2, None, None).await?;
    let user2 = test.user().insert_user(char2.id).await?;

    let original_hash = original_ownership.owner_hash.clone();
    let new_hash = "completely_different_hash";

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::transfer_character(&txn, character_model.id, user2.id, new_hash)
            .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.owner_hash, new_hash);
    assert_ne!(ownership.owner_hash, original_hash);

    txn.commit().await?;

    Ok(())
}

/// Tests error for unowned character.
///
/// Verifies that attempting to transfer a character that has no ownership
/// record results in the appropriate error.
///
/// Expected: Err(AuthError::CharacterNotOwned)
#[tokio::test]
async fn fails_for_unowned_character() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create character without ownership
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

    // Create a user to transfer to
    let char2 = test.eve().insert_mock_character(2, 2, None, None).await?;
    let user = test.user().insert_user(char2.id).await?;

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::transfer_character(&txn, character_model.id, user.id, "new_hash")
            .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        Error::AuthError(err) => {
            assert_eq!(
                format!("{:?}", err),
                format!(
                    "{:?}",
                    bifrost::server::error::auth::AuthError::CharacterNotOwned
                )
            );
        }
        _ => panic!("Expected AuthError::CharacterNotOwned"),
    }

    txn.rollback().await?;

    Ok(())
}

/// Tests transaction rollback prevents transfer.
///
/// Verifies that if the transaction is rolled back, the character
/// transfer does not persist to the database.
///
/// Expected: Original ownership preserved after rollback
#[tokio::test]
async fn rolls_back_on_transaction_abort() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user1, _, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let char2 = test.eve().insert_mock_character(2, 2, None, None).await?;
    let user2 = test.user().insert_user(char2.id).await?;

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::transfer_character(&txn, character_model.id, user2.id, "new_hash")
            .await;

    assert!(result.is_ok());

    // Rollback instead of commit
    txn.rollback().await?;

    // Verify ownership still belongs to user1
    let ownership = entity::prelude::BifrostUserCharacter::find_by_id(character_model.id)
        .one(&test.db)
        .await?
        .expect("Ownership should exist");
    assert_eq!(ownership.user_id, user1.id);

    // Verify user1 still exists
    let user1_check = entity::prelude::BifrostUser::find_by_id(user1.id)
        .one(&test.db)
        .await?;
    assert!(user1_check.is_some());

    Ok(())
}

/// Tests transferring multiple characters sequentially.
///
/// Verifies that multiple character transfers can be performed
/// successfully within the same transaction or sequentially.
///
/// Expected: Ok with all characters transferred correctly
#[tokio::test]
async fn transfers_multiple_characters_sequentially() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // User1 with two characters
    let (user1, _, char1) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (_, char2) = test
        .user()
        .insert_mock_character_for_user(user1.id, 2, 2, None, None)
        .await?;

    // User2 to receive the characters
    let char3 = test.eve().insert_mock_character(3, 3, None, None).await?;
    let user2 = test.user().insert_user(char3.id).await?;

    // Transfer first character
    let txn1 = test.db.begin().await?;
    let result1 =
        UserCharacterService::transfer_character(&txn1, char1.id, user2.id, "hash1").await;
    assert!(result1.is_ok());
    txn1.commit().await?;

    // Transfer second character
    let txn2 = test.db.begin().await?;
    let result2 =
        UserCharacterService::transfer_character(&txn2, char2.id, user2.id, "hash2").await;
    assert!(result2.is_ok());
    txn2.commit().await?;

    // Verify both characters belong to user2
    let ownership1 = entity::prelude::BifrostUserCharacter::find_by_id(char1.id)
        .one(&test.db)
        .await?
        .expect("Ownership1 should exist");
    assert_eq!(ownership1.user_id, user2.id);

    let ownership2 = entity::prelude::BifrostUserCharacter::find_by_id(char2.id)
        .one(&test.db)
        .await?
        .expect("Ownership2 should exist");
    assert_eq!(ownership2.user_id, user2.id);

    // Verify user1 was deleted (had no characters left)
    let user1_check = entity::prelude::BifrostUser::find_by_id(user1.id)
        .one(&test.db)
        .await?;
    assert!(user1_check.is_none());

    Ok(())
}

/// Tests that timestamps are updated during transfer.
///
/// Verifies that the ownership record's updated_at timestamp is
/// refreshed when a transfer occurs.
///
/// Expected: Ok with updated_at timestamp changed
#[tokio::test]
async fn updates_timestamp() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (_user1, original_ownership, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let char2 = test.eve().insert_mock_character(2, 2, None, None).await?;
    let user2 = test.user().insert_user(char2.id).await?;

    let original_updated_at = original_ownership.updated_at;

    // Wait to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::transfer_character(&txn, character_model.id, user2.id, "new_hash")
            .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();

    assert!(
        ownership.updated_at > original_updated_at,
        "updated_at should be newer after transfer"
    );

    txn.commit().await?;

    Ok(())
}

/// Tests transferring character with multiple owners in database.
///
/// Verifies correct behavior when multiple users exist and ensures
/// the transfer only affects the relevant users.
///
/// Expected: Ok with correct transfer, other users unaffected
#[tokio::test]
async fn transfers_with_multiple_users_in_database() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user1 with character to transfer
    let (_user1, _, char1) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Create user2 to receive the character
    let (user2, _, _) = test
        .user()
        .insert_user_with_mock_character(2, 2, None, None)
        .await?;

    // Create user3 as an unrelated user
    let (user3, _, _) = test
        .user()
        .insert_user_with_mock_character(3, 3, None, None)
        .await?;

    let txn = test.db.begin().await?;

    // Transfer char1 from user1 to user2
    let result =
        UserCharacterService::transfer_character(&txn, char1.id, user2.id, "new_hash").await;

    assert!(result.is_ok());
    txn.commit().await?;

    // Verify char1 now belongs to user2
    let ownership = entity::prelude::BifrostUserCharacter::find_by_id(char1.id)
        .one(&test.db)
        .await?
        .expect("Ownership should exist");
    assert_eq!(ownership.user_id, user2.id);

    // Verify user3 is unaffected
    let user3_check = entity::prelude::BifrostUser::find_by_id(user3.id)
        .one(&test.db)
        .await?;
    assert!(user3_check.is_some());

    Ok(())
}

/// Tests preserving created_at timestamp on transfer.
///
/// Verifies that the ownership record's created_at timestamp is not
/// modified during the transfer (only updated_at should change).
///
/// Expected: Ok with created_at unchanged
#[tokio::test]
async fn preserves_created_at_timestamp() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (_user1, original_ownership, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let char2 = test.eve().insert_mock_character(2, 2, None, None).await?;
    let user2 = test.user().insert_user(char2.id).await?;

    let original_created_at = original_ownership.created_at;

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::transfer_character(&txn, character_model.id, user2.id, "new_hash")
            .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();

    assert_eq!(
        ownership.created_at, original_created_at,
        "created_at should not change during transfer"
    );

    txn.commit().await?;

    Ok(())
}

/// Tests transfer with empty owner hash.
///
/// Verifies that transfers work correctly even with an empty owner hash
/// string (edge case for testing or specific scenarios).
///
/// Expected: Ok with empty owner hash set
#[tokio::test]
async fn handles_empty_owner_hash() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (_user1, _, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let char2 = test.eve().insert_mock_character(2, 2, None, None).await?;
    let user2 = test.user().insert_user(char2.id).await?;

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::transfer_character(&txn, character_model.id, user2.id, "").await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.owner_hash, "");

    txn.commit().await?;

    Ok(())
}

/// Tests transfer chain scenario.
///
/// Verifies that a character can be transferred through multiple users
/// in sequence (user1 -> user2 -> user3).
///
/// Expected: Ok with final ownership at user3
#[tokio::test]
async fn handles_transfer_chain() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create three users
    let (_user1, _, char1) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (user2, _, _) = test
        .user()
        .insert_user_with_mock_character(2, 2, None, None)
        .await?;
    let (user3, _, _) = test
        .user()
        .insert_user_with_mock_character(3, 3, None, None)
        .await?;

    // Transfer user1 -> user2
    let txn1 = test.db.begin().await?;
    let result1 =
        UserCharacterService::transfer_character(&txn1, char1.id, user2.id, "hash1").await;
    assert!(result1.is_ok());
    txn1.commit().await?;

    // Transfer user2 -> user3
    let txn2 = test.db.begin().await?;
    let result2 =
        UserCharacterService::transfer_character(&txn2, char1.id, user3.id, "hash2").await;
    assert!(result2.is_ok());
    txn2.commit().await?;

    // Verify final ownership is with user3
    let ownership = entity::prelude::BifrostUserCharacter::find_by_id(char1.id)
        .one(&test.db)
        .await?
        .expect("Ownership should exist");
    assert_eq!(ownership.user_id, user3.id);

    Ok(())
}
