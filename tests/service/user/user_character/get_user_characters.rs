//! Tests for UserCharacterService::get_user_characters method.
//!
//! This module verifies the user character retrieval service behavior, including
//! successful retrieval of characters with and without alliances, handling multiple
//! characters per user, timestamp validation, and edge cases for nonexistent users.

use bifrost::server::service::user::user_character::UserCharacterService;
use bifrost_test_utils::prelude::*;
use sea_orm::EntityTrait;

/// Tests retrieving a character without alliance.
///
/// Verifies that the user character service successfully retrieves a character
/// DTO for a user whose character is not affiliated with an alliance.
///
/// Expected: Ok with Vec containing one character DTO with null alliance
#[tokio::test]
async fn returns_character_without_alliance() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let (user_model, _, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 1);

    let character_dto = &character_dtos[0];
    assert_eq!(character_dto.id, character_model.character_id);
    assert_eq!(character_dto.name, character_model.name);
    assert_eq!(character_dto.corporation.id, 1);
    assert!(character_dto.alliance.is_none());

    Ok(())
}

/// Tests retrieving a character with alliance.
///
/// Verifies that the user character service successfully retrieves a character
/// DTO for a user whose character is affiliated with an alliance, including
/// complete alliance information with all fields correctly mapped.
///
/// Expected: Ok with Vec containing one character DTO with complete alliance data
#[tokio::test]
async fn returns_character_with_alliance() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
    let corporation_model = test.eve().insert_mock_corporation(1, Some(1), None).await?;
    let (user_model, _, character_model) = test
        .user()
        .insert_user_with_mock_character(
            1,
            corporation_model.corporation_id,
            Some(alliance_model.alliance_id),
            None,
        )
        .await?;

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 1);

    let character_dto = &character_dtos[0];
    assert_eq!(character_dto.id, character_model.character_id);
    assert_eq!(character_dto.name, character_model.name);
    assert_eq!(character_dto.corporation.id, 1);
    assert!(character_dto.alliance.is_some());

    // Verify all alliance DTO fields match database records
    let alliance_dto = character_dto.alliance.as_ref().unwrap();
    assert_eq!(alliance_dto.id, alliance_model.alliance_id);
    assert_eq!(alliance_dto.name, alliance_model.name);
    assert_eq!(alliance_dto.updated_at, alliance_model.updated_at);

    Ok(())
}

/// Tests retrieving multiple characters for a user.
///
/// Verifies that the user character service successfully retrieves all character
/// DTOs associated with a user who has multiple characters linked to their account.
///
/// Expected: Ok with Vec containing two character DTOs
#[tokio::test]
async fn returns_multiple_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let (user_model, _, character_model_1) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (_, character_model_2) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 2, Some(1), None)
        .await?;

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 2);

    // Validate both characters are present
    let char_ids: Vec<i64> = character_dtos.iter().map(|dto| dto.id).collect();
    assert!(char_ids.contains(&character_model_1.character_id));
    assert!(char_ids.contains(&character_model_2.character_id));

    // Validate all have corporations
    for character_dto in &character_dtos {
        assert!(character_dto.corporation.id > 0);
        assert!(!character_dto.corporation.name.is_empty());
    }

    Ok(())
}

/// Tests retrieving characters for user without any characters.
///
/// Verifies that the user character service returns an empty list when querying
/// a valid user who has no characters linked to their account.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_user_without_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    let user_model = test.user().insert_user(character_model.id).await?;

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 0);

    Ok(())
}

/// Tests retrieving characters for nonexistent user.
///
/// Verifies that the user character service returns an empty list when attempting
/// to retrieve characters for a user ID that does not exist in the database.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_nonexistent_user() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let nonexistent_user_id = 1;
    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(nonexistent_user_id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 0);

    Ok(())
}

/// Tests character DTOs contain valid timestamps.
///
/// Verifies that the user character service returns character DTOs with correctly
/// populated timestamp fields for character info, affiliation, corporation info,
/// and corporation affiliation updates.
///
/// Expected: Ok with character DTOs containing recent timestamps
#[tokio::test]
async fn returns_characters_with_timestamps() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 1);

    let character_dto = &character_dtos[0];

    let now = chrono::Utc::now().naive_utc();
    let tolerance = chrono::Duration::seconds(5); // Allow 5 second window

    // Ensure timestamp are populated and recent
    assert!(
        character_dto.info_updated_at <= now && character_dto.info_updated_at >= now - tolerance,
        "Character info_updated_at should be recent"
    );
    assert!(
        character_dto.affiliation_updated_at <= now
            && character_dto.affiliation_updated_at >= now - tolerance,
        "Character affiliation_updated_at should be recent"
    );
    assert!(
        character_dto.corporation.info_updated_at <= now
            && character_dto.corporation.info_updated_at >= now - tolerance,
        "Corporation info_updated_at should be recent"
    );
    assert!(
        character_dto.corporation.affiliation_updated_at <= now
            && character_dto.corporation.affiliation_updated_at >= now - tolerance,
        "Corporation affiliation_updated_at should be recent"
    );

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the user character service returns an error when attempting to
/// retrieve characters without the required database tables being created.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    // Use test setup that doesn't setup required tables, causing a database error
    let test = TestBuilder::new().build().await?;

    let nonexistent_user_id = 1;
    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(nonexistent_user_id)
        .await;

    assert!(result.is_err());

    Ok(())
}

/// Tests retrieving characters with mixed alliance states.
///
/// Verifies that the user character service correctly handles a user with multiple
/// characters where some are in alliances and others are not.
///
/// Expected: Ok with Vec containing characters with correct alliance states
#[tokio::test]
async fn returns_characters_with_mixed_alliance_states() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create first character without alliance
    let (user_model, _, char1) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Create second character with alliance
    let alliance_model = test.eve().insert_mock_alliance(2, None).await?;
    let corporation_model = test
        .eve()
        .insert_mock_corporation(2, Some(alliance_model.alliance_id), None)
        .await?;
    let (_, char2) = test
        .user()
        .insert_mock_character_for_user(
            user_model.id,
            2,
            corporation_model.corporation_id,
            Some(alliance_model.alliance_id),
            None,
        )
        .await?;

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 2);

    // Find each character and verify alliance state
    let dto1 = character_dtos
        .iter()
        .find(|dto| dto.id == char1.character_id)
        .expect("Character 1 should be present");
    assert!(
        dto1.alliance.is_none(),
        "Character 1 should have no alliance"
    );

    let dto2 = character_dtos
        .iter()
        .find(|dto| dto.id == char2.character_id)
        .expect("Character 2 should be present");
    assert!(
        dto2.alliance.is_some(),
        "Character 2 should have an alliance"
    );
    assert_eq!(
        dto2.alliance.as_ref().unwrap().id,
        alliance_model.alliance_id
    );

    Ok(())
}

/// Tests retrieving characters with special character names.
///
/// Verifies that the user character service correctly handles character names
/// with special characters, apostrophes, hyphens, and brackets.
///
/// Expected: Ok with Vec containing character with exact special name
#[tokio::test]
async fn handles_special_character_names() -> Result<(), TestError> {
    use chrono::Utc;
    use sea_orm::{ActiveValue, EntityTrait};

    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let character_id = 987654321;
    let corporation_id = 1;
    let special_name = "Test O'Brien-Smith [SPEC]";

    // Insert corporation first
    let corporation_model = test
        .eve()
        .insert_mock_corporation(corporation_id, None, None)
        .await?;

    // Insert character with special name manually
    let character_model =
        entity::prelude::EveCharacter::insert(entity::eve_character::ActiveModel {
            character_id: ActiveValue::Set(character_id),
            corporation_id: ActiveValue::Set(corporation_model.id),
            faction_id: ActiveValue::Set(None),
            birthday: ActiveValue::Set(Utc::now().naive_utc()),
            bloodline_id: ActiveValue::Set(1),
            description: ActiveValue::Set(Some("Test".to_string())),
            gender: ActiveValue::Set("male".to_string()),
            name: ActiveValue::Set(special_name.to_string()),
            race_id: ActiveValue::Set(1),
            security_status: ActiveValue::Set(Some(0.0)),
            title: ActiveValue::Set(None),
            created_at: ActiveValue::Set(Utc::now().naive_utc()),
            info_updated_at: ActiveValue::Set(Utc::now().naive_utc()),
            affiliation_updated_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        })
        .exec_with_returning(&test.db)
        .await?;

    let user_model = test.user().insert_user(character_model.id).await?;
    test.user()
        .insert_user_character_ownership(user_model.id, character_model.id)
        .await?;

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 1);
    assert_eq!(character_dtos[0].name, special_name);

    Ok(())
}

/// Tests retrieving characters with very large IDs.
///
/// Verifies that the user character service correctly handles characters
/// and corporations with very large i64 IDs.
///
/// Expected: Ok with Vec containing character with large IDs
#[tokio::test]
async fn handles_large_character_and_corporation_ids() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let large_character_id = 2_147_483_647_i64; // Near i32::MAX
    let large_corporation_id = 98_000_000_i64;

    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(large_character_id, large_corporation_id, None, None)
        .await?;

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 1);
    assert_eq!(character_dtos[0].id, large_character_id);
    assert_eq!(character_dtos[0].corporation.id, large_corporation_id);

    Ok(())
}

/// Tests retrieving characters in the same corporation.
///
/// Verifies that the user character service correctly handles multiple
/// characters belonging to the same corporation.
///
/// Expected: Ok with Vec containing characters with same corporation ID
#[tokio::test]
async fn handles_multiple_characters_in_same_corporation() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let corporation_id = 98_000_001;
    let (user_model, _, char1) = test
        .user()
        .insert_user_with_mock_character(1, corporation_id, None, None)
        .await?;
    let (_, char2) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, corporation_id, None, None)
        .await?;

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 2);

    // Verify both characters have the same corporation
    assert_eq!(
        character_dtos[0].corporation.id,
        character_dtos[1].corporation.id
    );
    assert_eq!(character_dtos[0].corporation.id, corporation_id);

    // Verify characters are distinct
    let char_ids: Vec<i64> = character_dtos.iter().map(|dto| dto.id).collect();
    assert!(char_ids.contains(&char1.character_id));
    assert!(char_ids.contains(&char2.character_id));

    Ok(())
}

/// Tests retrieving many characters for a single user.
///
/// Verifies that the user character service can handle users with many
/// characters (10+) and returns all of them correctly.
///
/// Expected: Ok with Vec containing all 10 characters
#[tokio::test]
async fn handles_many_characters_per_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user with first character
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Add 9 more characters
    let mut expected_char_ids = vec![1i64];
    for i in 2..=10 {
        let (_, char_model) = test
            .user()
            .insert_mock_character_for_user(user_model.id, i, i, None, None)
            .await?;
        expected_char_ids.push(char_model.character_id);
    }

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 10);

    // Verify all expected characters are present
    let returned_char_ids: Vec<i64> = character_dtos.iter().map(|dto| dto.id).collect();
    for expected_id in expected_char_ids {
        assert!(
            returned_char_ids.contains(&expected_id),
            "Character ID {} should be present",
            expected_id
        );
    }

    Ok(())
}

/// Tests retrieving characters for negative user ID.
///
/// Verifies that the user character service handles negative user IDs
/// correctly, returning an empty list.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_negative_user_id() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service.get_user_characters(-1).await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 0);

    Ok(())
}

/// Tests that character DTO data exactly matches database records.
///
/// Verifies complete data integrity by comparing all DTO fields against
/// the original database records for both character and corporation data.
///
/// Expected: Ok with DTO data matching database records exactly
#[tokio::test]
async fn dto_data_matches_database_records() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, _, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Get corporation from database
    let corporation_model = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .expect("Corporation should exist");

    let user_character_service = UserCharacterService::new(&test.db);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 1);

    let dto = &character_dtos[0];

    // Verify character fields
    assert_eq!(dto.id, character_model.character_id);
    assert_eq!(dto.name, character_model.name);
    assert_eq!(dto.info_updated_at, character_model.info_updated_at);
    assert_eq!(
        dto.affiliation_updated_at,
        character_model.affiliation_updated_at
    );

    // Verify corporation fields
    assert_eq!(dto.corporation.id, corporation_model.corporation_id);
    assert_eq!(dto.corporation.name, corporation_model.name);
    assert_eq!(
        dto.corporation.info_updated_at,
        corporation_model.info_updated_at
    );
    assert_eq!(
        dto.corporation.affiliation_updated_at,
        corporation_model.affiliation_updated_at
    );

    Ok(())
}
