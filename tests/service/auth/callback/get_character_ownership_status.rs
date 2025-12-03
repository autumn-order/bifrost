//! Tests for CallbackService::get_character_ownership_status method.
//!
//! This module verifies the character ownership status retrieval logic,
//! including handling of non-existent characters, unowned characters,
//! and owned characters with proper database lookups.

use bifrost::server::{
    error::Error,
    service::auth::callback::{CallbackService, CharacterRecord},
};
use bifrost_test_utils::prelude::*;

/// Tests retrieving ownership status for a non-existent character.
///
/// Verifies that when a character ID does not exist in the database,
/// the method returns CharacterRecord::NotFound.
///
/// Expected: Ok(CharacterRecord::NotFound)
#[tokio::test]
async fn returns_not_found_for_nonexistent_character() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let nonexistent_character_id = 999999999;

    let result =
        CallbackService::get_character_ownership_status(&test.db, nonexistent_character_id).await;

    assert!(result.is_ok());
    match result.unwrap() {
        CharacterRecord::NotFound => {} // Expected
        _ => panic!("Expected CharacterRecord::NotFound"),
    }

    Ok(())
}

/// Tests retrieving ownership status for an unowned character.
///
/// Verifies that when a character exists in the database but has no
/// ownership record (not linked to any user), the method returns
/// CharacterRecord::Unowned with the character model.
///
/// Expected: Ok(CharacterRecord::Unowned)
#[tokio::test]
async fn returns_unowned_for_character_without_owner() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Insert a character without creating an ownership record
    let character_id = 123456789;
    let character_model = test
        .eve()
        .insert_mock_character(character_id, 1, None, None)
        .await?;

    let result = CallbackService::get_character_ownership_status(&test.db, character_id).await;

    assert!(result.is_ok());
    match result.unwrap() {
        CharacterRecord::Unowned { character } => {
            assert_eq!(character.id, character_model.id);
            assert_eq!(character.character_id, character_id);
        }
        _ => panic!("Expected CharacterRecord::Unowned"),
    }

    Ok(())
}

/// Tests retrieving ownership status for an owned character.
///
/// Verifies that when a character exists in the database and has an
/// ownership record linking it to a user, the method returns
/// CharacterRecord::Owned with both character and ownership models.
///
/// Expected: Ok(CharacterRecord::Owned)
#[tokio::test]
async fn returns_owned_for_character_with_owner() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create a user with a character (creates both character and ownership records)
    let character_id = 123456789;
    let (user_model, ownership_model, character_model) = test
        .user()
        .insert_user_with_mock_character(character_id, 1, None, None)
        .await?;

    let result = CallbackService::get_character_ownership_status(&test.db, character_id).await;

    assert!(result.is_ok());
    match result.unwrap() {
        CharacterRecord::Owned {
            character,
            ownership,
        } => {
            assert_eq!(character.id, character_model.id);
            assert_eq!(character.character_id, character_id);
            assert_eq!(ownership.id, ownership_model.id);
            assert_eq!(ownership.user_id, user_model.id);
            assert_eq!(ownership.character_id, character_model.id);
        }
        _ => panic!("Expected CharacterRecord::Owned"),
    }

    Ok(())
}

/// Tests retrieving ownership status for multiple characters with different states.
///
/// Verifies that the method correctly distinguishes between not found,
/// unowned, and owned characters when multiple characters exist in various states.
///
/// Expected: Correct CharacterRecord variant for each character
#[tokio::test]
async fn handles_multiple_characters_with_different_states() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let nonexistent_id = 999999999;
    let unowned_id = 111111111;
    let owned_id = 222222222;

    // Insert unowned character
    test.eve()
        .insert_mock_character(unowned_id, 1, None, None)
        .await?;

    // Insert owned character
    test.user()
        .insert_user_with_mock_character(owned_id, 2, None, None)
        .await?;

    // Check non-existent
    let result1 = CallbackService::get_character_ownership_status(&test.db, nonexistent_id).await;
    assert!(matches!(result1.unwrap(), CharacterRecord::NotFound));

    // Check unowned
    let result2 = CallbackService::get_character_ownership_status(&test.db, unowned_id).await;
    assert!(matches!(result2.unwrap(), CharacterRecord::Unowned { .. }));

    // Check owned
    let result3 = CallbackService::get_character_ownership_status(&test.db, owned_id).await;
    assert!(matches!(result3.unwrap(), CharacterRecord::Owned { .. }));

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the method returns a database error when attempting to
/// query character ownership without the required database tables.
///
/// Expected: Err(Error::DbErr)
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let character_id = 123456789;

    let result = CallbackService::get_character_ownership_status(&test.db, character_id).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::DbErr(_)));

    Ok(())
}

/// Tests retrieving ownership status after character transfer.
///
/// Verifies that when a character's ownership is transferred from one user
/// to another, the method returns the updated ownership information.
///
/// Expected: Ok(CharacterRecord::Owned) with new owner's information
#[tokio::test]
async fn returns_updated_ownership_after_transfer() -> Result<(), TestError> {
    use bifrost::server::service::user::user_character::UserCharacterService;
    use sea_orm::TransactionTrait;

    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_id = 123456789;

    // Create first user with character
    let (user1, ownership1, character) = test
        .user()
        .insert_user_with_mock_character(character_id, 1, None, None)
        .await?;

    // Verify initial ownership
    let result1 = CallbackService::get_character_ownership_status(&test.db, character_id).await;
    match result1.unwrap() {
        CharacterRecord::Owned { ownership, .. } => {
            assert_eq!(ownership.user_id, user1.id);
        }
        _ => panic!("Expected CharacterRecord::Owned"),
    }

    // Create second user with a different character
    let (user2, _, _) = test
        .user()
        .insert_user_with_mock_character(987654321, 2, None, None)
        .await?;

    // Transfer character ownership using the service
    let txn = test.db.begin().await?;
    UserCharacterService::transfer_character(&txn, character.id, user2.id, "new_owner_hash")
        .await
        .unwrap();
    txn.commit().await?;

    // Verify updated ownership
    let result2 = CallbackService::get_character_ownership_status(&test.db, character_id).await;
    match result2.unwrap() {
        CharacterRecord::Owned { ownership, .. } => {
            assert_eq!(ownership.user_id, user2.id);
            assert_eq!(ownership.id, ownership1.id); // Same ownership record, updated
            assert_eq!(ownership.owner_hash, "new_owner_hash");
        }
        _ => panic!("Expected CharacterRecord::Owned"),
    }

    Ok(())
}

/// Tests retrieving ownership status for characters with same corporation.
///
/// Verifies that the method correctly handles multiple characters from the
/// same corporation, ensuring each character's ownership status is tracked
/// independently.
///
/// Expected: Correct ownership status for each character
#[tokio::test]
async fn handles_multiple_characters_from_same_corporation() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let corporation_id = 1;
    let character_id_1 = 111111111;
    let character_id_2 = 222222222;

    // Create two characters in same corporation with different ownership states
    test.eve()
        .insert_mock_character(character_id_1, corporation_id, None, None)
        .await?;

    test.user()
        .insert_user_with_mock_character(character_id_2, corporation_id, None, None)
        .await?;

    // First character is unowned
    let result1 = CallbackService::get_character_ownership_status(&test.db, character_id_1).await;
    assert!(matches!(result1.unwrap(), CharacterRecord::Unowned { .. }));

    // Second character is owned
    let result2 = CallbackService::get_character_ownership_status(&test.db, character_id_2).await;
    assert!(matches!(result2.unwrap(), CharacterRecord::Owned { .. }));

    Ok(())
}

/// Tests retrieving ownership status with various character IDs.
///
/// Verifies that the method correctly handles different character ID formats
/// including small and large i64 values.
///
/// Expected: Ok with correct CharacterRecord for each ID
#[tokio::test]
async fn handles_various_character_ids() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let test_cases = vec![
        1i64,             // Minimum test value
        123456789i64,     // Common format
        2147483647i64,    // Max i32
        9999999999999i64, // Large value
    ];

    for (idx, character_id) in test_cases.iter().enumerate() {
        let corp_id = (idx + 1) as i64;
        test.user()
            .insert_user_with_mock_character(*character_id, corp_id, None, None)
            .await?;

        let result = CallbackService::get_character_ownership_status(&test.db, *character_id).await;

        assert!(result.is_ok());
        match result.unwrap() {
            CharacterRecord::Owned { character, .. } => {
                assert_eq!(character.character_id, *character_id);
            }
            _ => panic!(
                "Expected CharacterRecord::Owned for character {}",
                character_id
            ),
        }
    }

    Ok(())
}
