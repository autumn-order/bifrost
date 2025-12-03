//! Tests for UserCharacterService::get_user_characters method.
//!
//! This module verifies the user character retrieval service behavior, including
//! successful retrieval of characters with and without alliances, handling multiple
//! characters per user, timestamp validation, and edge cases for nonexistent users.

use bifrost::server::service::user::user_character::UserCharacterService;
use bifrost_test_utils::prelude::*;

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
/// complete alliance information.
///
/// Expected: Ok with Vec containing one character DTO with alliance data
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
    let alliance_dto = character_dto.alliance.as_ref().unwrap();
    assert_eq!(alliance_dto.id, alliance_model.alliance_id);
    assert_eq!(alliance_dto.name, alliance_model.name);

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
