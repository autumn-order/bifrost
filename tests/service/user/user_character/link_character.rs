//! Tests for UserCharacterService::link_character method.
//!
//! This module verifies the character linking service behavior, including creating
//! new ownership records, updating existing ownership, handling owner hash updates,
//! and transaction rollback behavior.

use bifrost::server::service::user::user_character::UserCharacterService;
use bifrost_test_utils::prelude::*;
use sea_orm::{EntityTrait, TransactionTrait};

/// Tests linking a character to a user for the first time.
///
/// Verifies that the service successfully creates a new ownership record
/// when linking a character that has no existing ownership.
///
/// Expected: Ok with new ownership record created
#[tokio::test]
async fn creates_new_ownership_record() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    let user_model = test.user().insert_user(character_model.id).await?;

    let txn = test.db.begin().await?;

    let owner_hash = "test_owner_hash_123";
    let result =
        UserCharacterService::link_character(&txn, character_model.id, user_model.id, owner_hash)
            .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.character_id, character_model.id);
    assert_eq!(ownership.user_id, user_model.id);
    assert_eq!(ownership.owner_hash, owner_hash);

    txn.commit().await?;

    // Verify ownership exists in database
    let db_ownership = entity::prelude::BifrostUserCharacter::find_by_id(character_model.id)
        .one(&test.db)
        .await?;
    assert!(db_ownership.is_some());
    assert_eq!(db_ownership.unwrap().user_id, user_model.id);

    Ok(())
}

/// Tests updating existing ownership record.
///
/// Verifies that the service updates an existing ownership record when
/// a character is already linked to a user.
///
/// Expected: Ok with ownership record updated to new user
#[tokio::test]
async fn updates_existing_ownership() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create first user with character
    let (_user1, ownership1, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Create second user
    let char2 = test.eve().insert_mock_character(2, 2, None, None).await?;
    let user2 = test.user().insert_user(char2.id).await?;

    let original_owner_hash = ownership1.owner_hash.clone();
    let new_owner_hash = "new_owner_hash_456";

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::link_character(&txn, character_model.id, user2.id, new_owner_hash)
            .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.character_id, character_model.id);
    assert_eq!(ownership.user_id, user2.id);
    assert_eq!(ownership.owner_hash, new_owner_hash);
    assert_ne!(ownership.owner_hash, original_owner_hash);

    txn.commit().await?;

    // Verify ownership was updated in database
    let db_ownership = entity::prelude::BifrostUserCharacter::find_by_id(character_model.id)
        .one(&test.db)
        .await?
        .expect("Ownership should exist");
    assert_eq!(db_ownership.user_id, user2.id);
    assert_eq!(db_ownership.owner_hash, new_owner_hash);

    Ok(())
}

/// Tests updating owner hash for same user.
///
/// Verifies that the service updates the owner hash when linking a character
/// to the same user it's already linked to (re-authentication scenario).
///
/// Expected: Ok with owner hash updated
#[tokio::test]
async fn updates_owner_hash_for_same_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, ownership_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let original_owner_hash = ownership_model.owner_hash.clone();
    let new_owner_hash = "refreshed_owner_hash_789";

    let txn = test.db.begin().await?;

    let result = UserCharacterService::link_character(
        &txn,
        character_model.id,
        user_model.id,
        new_owner_hash,
    )
    .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.character_id, character_model.id);
    assert_eq!(ownership.user_id, user_model.id);
    assert_eq!(ownership.owner_hash, new_owner_hash);
    assert_ne!(ownership.owner_hash, original_owner_hash);

    txn.commit().await?;

    Ok(())
}

/// Tests linking multiple characters to the same user.
///
/// Verifies that the service can link multiple different characters
/// to the same user account.
///
/// Expected: Ok with multiple ownership records for one user
#[tokio::test]
async fn links_multiple_characters_to_same_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let character2 = test.eve().insert_mock_character(2, 2, None, None).await?;
    let character3 = test.eve().insert_mock_character(3, 3, None, None).await?;

    let txn = test.db.begin().await?;

    // Link second character
    let result2 =
        UserCharacterService::link_character(&txn, character2.id, user_model.id, "owner_hash_2")
            .await;
    assert!(result2.is_ok());

    // Link third character
    let result3 =
        UserCharacterService::link_character(&txn, character3.id, user_model.id, "owner_hash_3")
            .await;
    assert!(result3.is_ok());

    txn.commit().await?;

    // Verify all three characters are owned by the user
    let ownerships = entity::prelude::BifrostUserCharacter::find()
        .all(&test.db)
        .await?;
    let user_ownerships: Vec<_> = ownerships
        .into_iter()
        .filter(|o| o.user_id == user_model.id)
        .collect();
    assert_eq!(user_ownerships.len(), 3);

    Ok(())
}

/// Tests transaction rollback on error.
///
/// Verifies that if the transaction is not committed, the ownership
/// record is not persisted to the database.
///
/// Expected: No ownership record after rollback
#[tokio::test]
async fn rolls_back_on_transaction_abort() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    let user_model = test.user().insert_user(character_model.id).await?;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::link_character(
        &txn,
        character_model.id,
        user_model.id,
        "test_owner_hash",
    )
    .await;

    assert!(result.is_ok());

    // Explicitly rollback transaction
    txn.rollback().await?;

    // Verify ownership does NOT exist in database
    let db_ownership = entity::prelude::BifrostUserCharacter::find_by_id(character_model.id)
        .one(&test.db)
        .await?;
    assert!(db_ownership.is_none());

    Ok(())
}

/// Tests linking with empty owner hash.
///
/// Verifies that the service accepts an empty owner hash string
/// (edge case that might occur in testing or specific scenarios).
///
/// Expected: Ok with empty owner hash stored
#[tokio::test]
async fn accepts_empty_owner_hash() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    let user_model = test.user().insert_user(character_model.id).await?;

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::link_character(&txn, character_model.id, user_model.id, "").await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.owner_hash, "");

    txn.commit().await?;

    Ok(())
}

/// Tests linking with very long owner hash.
///
/// Verifies that the service can handle owner hashes with realistic
/// lengths (EVE SSO owner hashes are base64 encoded strings).
///
/// Expected: Ok with long owner hash stored
#[tokio::test]
async fn handles_long_owner_hash() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    let user_model = test.user().insert_user(character_model.id).await?;

    // Simulate a realistic base64-encoded owner hash
    let long_owner_hash = "dGhpcyBpcyBhIHZlcnkgbG9uZyBvd25lciBoYXNoIHRoYXQgbWlnaHQgYmUgdXNlZCBpbiByZWFsIHNjZW5hcmlvcyB3aXRoIEVWRSBPbmxpbmUgU1NPIGF1dGhlbnRpY2F0aW9u";

    let txn = test.db.begin().await?;

    let result = UserCharacterService::link_character(
        &txn,
        character_model.id,
        user_model.id,
        long_owner_hash,
    )
    .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.owner_hash, long_owner_hash);

    txn.commit().await?;

    Ok(())
}

/// Tests linking with special characters in owner hash.
///
/// Verifies that the service handles owner hashes containing
/// special characters that might appear in base64 encoding.
///
/// Expected: Ok with special characters preserved
#[tokio::test]
async fn handles_special_characters_in_owner_hash() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    let user_model = test.user().insert_user(character_model.id).await?;

    let special_owner_hash = "abc123+/=XYZ789";

    let txn = test.db.begin().await?;

    let result = UserCharacterService::link_character(
        &txn,
        character_model.id,
        user_model.id,
        special_owner_hash,
    )
    .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.owner_hash, special_owner_hash);

    txn.commit().await?;

    Ok(())
}

/// Tests that updated_at timestamp is set.
///
/// Verifies that the ownership record has an updated_at timestamp
/// that reflects when the operation occurred.
///
/// Expected: Ok with recent updated_at timestamp
#[tokio::test]
async fn sets_updated_at_timestamp() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    let user_model = test.user().insert_user(character_model.id).await?;

    let before = chrono::Utc::now().naive_utc();

    let txn = test.db.begin().await?;

    let result =
        UserCharacterService::link_character(&txn, character_model.id, user_model.id, "owner_hash")
            .await;

    let after = chrono::Utc::now().naive_utc();

    assert!(result.is_ok());
    let ownership = result.unwrap();

    assert!(
        ownership.updated_at >= before,
        "updated_at should be >= operation start time"
    );
    assert!(
        ownership.updated_at <= after,
        "updated_at should be <= operation end time"
    );

    txn.commit().await?;

    Ok(())
}

/// Tests that updated_at changes on re-link.
///
/// Verifies that when ownership is updated, the updated_at timestamp
/// is refreshed to reflect the new operation time.
///
/// Expected: Ok with updated_at timestamp changed
#[tokio::test]
async fn updates_timestamp_on_relink() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, ownership_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let original_updated_at = ownership_model.updated_at;

    // Wait a moment to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::link_character(
        &txn,
        character_model.id,
        user_model.id,
        "new_owner_hash",
    )
    .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();

    assert!(
        ownership.updated_at > original_updated_at,
        "updated_at should be newer after re-linking"
    );

    txn.commit().await?;

    Ok(())
}

/// Tests linking nonexistent character fails appropriately.
///
/// Verifies that attempting to link a character that doesn't exist
/// in the database results in a database error due to foreign key constraint.
///
/// Expected: Err(DbErr)
#[tokio::test]
async fn fails_for_nonexistent_character() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    let user_model = test.user().insert_user(character_model.id).await?;

    let nonexistent_character_id = character_model.id + 999;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::link_character(
        &txn,
        nonexistent_character_id,
        user_model.id,
        "owner_hash",
    )
    .await;

    assert!(result.is_err());

    txn.rollback().await?;

    Ok(())
}

/// Tests linking to nonexistent user fails appropriately.
///
/// Verifies that attempting to link a character to a user that doesn't exist
/// in the database results in a database error due to foreign key constraint.
///
/// Expected: Err(DbErr)
#[tokio::test]
async fn fails_for_nonexistent_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

    let nonexistent_user_id = 999;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::link_character(
        &txn,
        character_model.id,
        nonexistent_user_id,
        "owner_hash",
    )
    .await;

    assert!(result.is_err());

    txn.rollback().await?;

    Ok(())
}

/// Tests that created_at is preserved on update.
///
/// Verifies that when an existing ownership record is updated,
/// the original created_at timestamp is not changed.
///
/// Expected: Ok with created_at unchanged
#[tokio::test]
async fn preserves_created_at_on_update() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, ownership_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let original_created_at = ownership_model.created_at;

    // Wait to ensure different timestamp if it were to change
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let txn = test.db.begin().await?;

    let result = UserCharacterService::link_character(
        &txn,
        character_model.id,
        user_model.id,
        "new_owner_hash",
    )
    .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();

    assert_eq!(
        ownership.created_at, original_created_at,
        "created_at should not change on update"
    );

    txn.commit().await?;

    Ok(())
}

/// Tests linking character from one user to another.
///
/// Verifies the complete flow of transferring a character's ownership
/// from one user to another user.
///
/// Expected: Ok with ownership transferred
#[tokio::test]
async fn transfers_ownership_between_users() -> Result<(), TestError> {
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

    // Transfer character from user1 to user2
    let result =
        UserCharacterService::link_character(&txn, character_model.id, user2.id, "new_owner_hash")
            .await;

    assert!(result.is_ok());
    let ownership = result.unwrap();
    assert_eq!(ownership.user_id, user2.id);
    assert_ne!(ownership.user_id, user1.id);

    txn.commit().await?;

    // Verify ownership in database belongs to user2
    let db_ownership = entity::prelude::BifrostUserCharacter::find_by_id(character_model.id)
        .one(&test.db)
        .await?
        .expect("Ownership should exist");
    assert_eq!(db_ownership.user_id, user2.id);

    Ok(())
}
