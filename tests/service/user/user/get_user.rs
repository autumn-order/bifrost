//! Tests for UserService::get_user method.
//!
//! This module verifies the user retrieval service behavior, including successful
//! retrieval of existing users with correct data mapping, handling of nonexistent
//! user IDs, multiple users, and error handling when required database tables are
//! missing or when foreign key constraints are violated.

use bifrost::server::{error::Error, service::user::UserService};
use bifrost_test_utils::prelude::*;

/// Tests retrieving an existing user.
///
/// Verifies that the user service successfully retrieves a user record from the
/// database when provided with a valid user ID.
///
/// Expected: Ok(Some(UserDto))
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

/// Tests that returned UserDto contains correct data.
///
/// Verifies that the user service correctly maps database records to UserDto,
/// including user ID, main character ID, and main character name.
///
/// Expected: Ok(Some(UserDto)) with correct fields
#[tokio::test]
async fn returns_user_dto_with_correct_fields() -> Result<(), TestError> {
    let character_id = 123456789;
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let (user_model, _, character_model) = test
        .user()
        .insert_user_with_mock_character(character_id, 1, None, None)
        .await?;

    let user_service = UserService::new(&test.db);
    let result = user_service.get_user(user_model.id).await;

    assert!(result.is_ok());
    let user_dto = result.unwrap().expect("User should exist");
    assert_eq!(user_dto.id, user_model.id);
    assert_eq!(user_dto.character_id, character_model.character_id);
    assert_eq!(user_dto.character_name, character_model.name);

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

/// Tests retrieving multiple different users.
///
/// Verifies that the user service can correctly retrieve different users
/// when multiple users exist in the database, ensuring proper isolation
/// of user records.
///
/// Expected: Ok(Some(UserDto)) for each user with correct data
#[tokio::test]
async fn retrieves_correct_user_among_multiple() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create three users with different characters
    let (user1, _, char1) = test
        .user()
        .insert_user_with_mock_character(111111111, 1, None, None)
        .await?;
    let (user2, _, char2) = test
        .user()
        .insert_user_with_mock_character(222222222, 2, None, None)
        .await?;
    let (user3, _, char3) = test
        .user()
        .insert_user_with_mock_character(333333333, 3, None, None)
        .await?;

    let user_service = UserService::new(&test.db);

    // Retrieve each user and verify correct data
    let result1 = user_service
        .get_user(user1.id)
        .await
        .expect("Service call failed");
    let dto1 = result1.expect("User 1 should exist");
    assert_eq!(dto1.id, user1.id);
    assert_eq!(dto1.character_id, char1.character_id);
    assert_eq!(dto1.character_name, char1.name);

    let result2 = user_service
        .get_user(user2.id)
        .await
        .expect("Service call failed");
    let dto2 = result2.expect("User 2 should exist");
    assert_eq!(dto2.id, user2.id);
    assert_eq!(dto2.character_id, char2.character_id);
    assert_eq!(dto2.character_name, char2.name);

    let result3 = user_service
        .get_user(user3.id)
        .await
        .expect("Service call failed");
    let dto3 = result3.expect("User 3 should exist");
    assert_eq!(dto3.id, user3.id);
    assert_eq!(dto3.character_id, char3.character_id);
    assert_eq!(dto3.character_name, char3.name);

    Ok(())
}

/// Tests retrieving user with various character name formats.
///
/// Verifies that the user service correctly handles character names with
/// special characters, spaces, and various lengths.
///
/// Expected: Ok(Some(UserDto)) with exact character name
#[tokio::test]
async fn handles_various_character_name_formats() -> Result<(), TestError> {
    use chrono::Utc;
    use sea_orm::{ActiveValue, EntityTrait};

    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create character with special name
    let character_id = 987654321;
    let corporation_id = 1;
    let special_name = "Test Character O'Brien-Smith [TLA]";

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

    let user_service = UserService::new(&test.db);
    let result = user_service
        .get_user(user_model.id)
        .await
        .expect("Service call failed");

    let user_dto = result.expect("User should exist");
    assert_eq!(user_dto.character_name, special_name);

    Ok(())
}

/// Tests retrieving user with negative user ID.
///
/// Verifies that the user service handles negative user IDs correctly,
/// returning None if no such user exists (negative IDs are technically valid
/// in the database schema but unlikely in practice).
///
/// Expected: Ok(None)
#[tokio::test]
async fn returns_none_for_negative_user_id() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let user_service = UserService::new(&test.db);
    let result = user_service.get_user(-1).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());

    Ok(())
}

/// Tests retrieving user with very large user ID.
///
/// Verifies that the user service handles large user IDs correctly,
/// returning None when no such user exists.
///
/// Expected: Ok(None)
#[tokio::test]
async fn returns_none_for_large_user_id() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let user_service = UserService::new(&test.db);
    let result = user_service.get_user(i32::MAX).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the user service returns a database error when attempting to
/// retrieve a user without the required database tables being created.
///
/// Expected: Err(Error::DbErr)
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let user_id = 1;
    let user_service = UserService::new(&test.db);
    let result = user_service.get_user(user_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests retrieving user after updating main character.
///
/// Verifies that the user service returns updated main character information
/// after the user's main character has been changed.
///
/// Expected: Ok(Some(UserDto)) with new main character data
#[tokio::test]
async fn returns_updated_main_character() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user with first character
    let (user_model, _, char1) = test
        .user()
        .insert_user_with_mock_character(111111111, 1, None, None)
        .await?;

    // Create second character
    let char2 = test
        .eve()
        .insert_mock_character(222222222, 2, None, None)
        .await?;

    // Update user's main character using repository
    use bifrost::server::data::user::UserRepository;
    let user_repo = UserRepository::new(&test.db);
    user_repo.update(user_model.id, char2.id).await?;

    // Retrieve user and verify new main character
    let user_service = UserService::new(&test.db);
    let result = user_service
        .get_user(user_model.id)
        .await
        .expect("Service call failed");

    let user_dto = result.expect("User should exist");
    assert_eq!(user_dto.id, user_model.id);
    assert_eq!(user_dto.character_id, char2.character_id);
    assert_eq!(user_dto.character_name, char2.name);
    assert_ne!(user_dto.character_id, char1.character_id);

    Ok(())
}
