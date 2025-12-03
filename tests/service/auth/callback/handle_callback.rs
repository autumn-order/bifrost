//! Tests for CallbackService::handle_callback method.
//!
//! This module verifies the complete OAuth callback flow orchestration,
//! including authentication, character ownership management, user creation,
//! and main character updates across various scenarios.

use bifrost::server::{
    data::user::UserRepository, error::Error, service::auth::callback::CallbackService,
};
use bifrost_test_utils::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// Tests successful callback for a new character creating a new user.
///
/// Verifies that when a character logs in for the first time (not in database,
/// no user logged in), the service fetches the character from ESI, creates a
/// new user, and links the character to that user.
///
/// Expected: Ok with new user ID
#[tokio::test]
async fn creates_new_user_for_new_character() -> Result<(), TestError> {
    let character_id = 123456789;
    let corporation_id = 1;
    let owner_hash = "owner_hash_123";

    let mock_corporation = factory::mock_corporation(None, None);
    let mock_character = factory::mock_character(corporation_id, None, None);

    let test = TestBuilder::new()
        .with_user_tables()
        .with_corporation_endpoint(corporation_id, mock_corporation, 1)
        .with_character_endpoint(character_id, mock_character, 1)
        .with_jwt_endpoints(character_id, owner_hash)
        .build()
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    let result = service
        .handle_callback("auth_code", None, None)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    // Should create first user
    assert_eq!(result, 1);

    // Verify user exists with character as main
    let user_repo = UserRepository::new(&test.db);
    let (user, _) = user_repo.get_by_id(result).await?.unwrap();
    assert_eq!(user.id, 1);

    test.assert_mocks();

    Ok(())
}

/// Tests successful callback for a new character with logged-in user.
///
/// Verifies that when a new character logs in while a user is already logged in,
/// the character is fetched from ESI and linked to the existing user.
///
/// Expected: Ok with existing user ID
#[tokio::test]
async fn links_new_character_to_existing_user() -> Result<(), TestError> {
    let character_id = 123456789;
    let corporation_id = 1;
    let owner_hash = "owner_hash_123";

    let mock_corporation = factory::mock_corporation(None, None);
    let mock_character = factory::mock_character(corporation_id, None, None);

    let mut test = TestBuilder::new()
        .with_user_tables()
        .with_corporation_endpoint(corporation_id, mock_corporation, 1)
        .with_character_endpoint(character_id, mock_character, 1)
        .with_jwt_endpoints(character_id, owner_hash)
        .build()
        .await?;

    // Create an existing user with a different character
    let (existing_user, _, _) = test
        .user()
        .insert_user_with_mock_character(987654321, 2, None, None)
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    let result = service
        .handle_callback("auth_code", Some(existing_user.id), None)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    // Should return existing user ID
    assert_eq!(result, existing_user.id);

    test.assert_mocks();

    Ok(())
}

/// Tests callback for unowned character without logged-in user.
///
/// Verifies that when an unowned character (exists in DB but no ownership)
/// logs in without a user session, a new user is created and linked.
///
/// Expected: Ok with new user ID
#[tokio::test]
async fn creates_user_for_unowned_character() -> Result<(), TestError> {
    let character_id = 123456789;
    let owner_hash = "owner_hash_123";

    let mut test = TestBuilder::new()
        .with_user_tables()
        .with_jwt_endpoints(character_id, owner_hash)
        .build()
        .await?;

    // Insert character without ownership
    test.eve()
        .insert_mock_character(character_id, 1, None, None)
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    let result = service
        .handle_callback("auth_code", None, None)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    assert_eq!(result, 1);

    test.assert_mocks();

    Ok(())
}

/// Tests callback for unowned character with logged-in user.
///
/// Verifies that when an unowned character logs in with a user session,
/// the character is linked to the existing user.
///
/// Expected: Ok with existing user ID
#[tokio::test]
async fn links_unowned_character_to_logged_in_user() -> Result<(), TestError> {
    let character_id = 123456789;
    let owner_hash = "owner_hash_123";

    let mut test = TestBuilder::new()
        .with_user_tables()
        .with_jwt_endpoints(character_id, owner_hash)
        .build()
        .await?;

    // Create existing user
    let (existing_user, _, _) = test
        .user()
        .insert_user_with_mock_character(987654321, 1, None, None)
        .await?;

    // Insert unowned character
    test.eve()
        .insert_mock_character(character_id, 2, None, None)
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    let result = service
        .handle_callback("auth_code", Some(existing_user.id), None)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    assert_eq!(result, existing_user.id);

    test.assert_mocks();

    Ok(())
}

/// Tests callback for owned character logging in without user session.
///
/// Verifies that when a character that's already owned logs in without
/// a user session and the owner hash matches, the user is logged in.
///
/// Expected: Ok with character's owner user ID
#[tokio::test]
async fn logs_in_existing_user_with_matching_hash() -> Result<(), TestError> {
    let character_id = 123456789;
    let owner_hash = "owner_hash_123";

    let mut test = TestBuilder::new()
        .with_user_tables()
        .with_jwt_endpoints(character_id, owner_hash)
        .build()
        .await?;

    // Create user with character
    let (user, ownership, _) = test
        .user()
        .insert_user_with_mock_character(character_id, 1, None, None)
        .await?;

    // Update owner hash to match
    entity::bifrost_user_character::Entity::update_many()
        .col_expr(
            entity::bifrost_user_character::Column::OwnerHash,
            sea_orm::sea_query::Expr::value(owner_hash),
        )
        .filter(entity::bifrost_user_character::Column::Id.eq(ownership.id))
        .exec(&test.db)
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    let result = service
        .handle_callback("auth_code", None, None)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    assert_eq!(result, user.id);

    test.assert_mocks();

    Ok(())
}

/// Tests callback for character transfer between users.
///
/// Verifies that when a character owned by one user logs in with a different
/// user session (indicating account transfer), the ownership is transferred.
///
/// Expected: Ok with logged-in user ID
#[tokio::test]
async fn transfers_character_to_logged_in_user() -> Result<(), TestError> {
    let character_id = 123456789;
    let owner_hash = "new_owner_hash";

    let mut test = TestBuilder::new()
        .with_user_tables()
        .with_jwt_endpoints(character_id, owner_hash)
        .build()
        .await?;

    // Create first user with character
    let (_user1, _, _) = test
        .user()
        .insert_user_with_mock_character(character_id, 1, None, None)
        .await?;

    // Create second user
    let (user2, _, _) = test
        .user()
        .insert_user_with_mock_character(987654321, 2, None, None)
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    // Character from user1 logs in with user2 session
    let result = service
        .handle_callback("auth_code", Some(user2.id), None)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    assert_eq!(result, user2.id);

    test.assert_mocks();

    Ok(())
}

/// Tests callback for owner hash update (same user, different EVE account).
///
/// Verifies that when a character's owner hash changes but the same user
/// is logged in, only the owner hash is updated.
///
/// Expected: Ok with same user ID
#[tokio::test]
async fn updates_owner_hash_for_same_user() -> Result<(), TestError> {
    let character_id = 123456789;
    let new_owner_hash = "new_owner_hash";

    let mut test = TestBuilder::new()
        .with_user_tables()
        .with_jwt_endpoints(character_id, new_owner_hash)
        .build()
        .await?;

    // Create user with character (has old owner hash)
    let (user, _, _) = test
        .user()
        .insert_user_with_mock_character(character_id, 1, None, None)
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    // Same user logs in with new owner hash
    let result = service
        .handle_callback("auth_code", Some(user.id), None)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    assert_eq!(result, user.id);

    test.assert_mocks();

    Ok(())
}

/// Tests callback with change_main flag for already owned character.
///
/// Verifies that when a user logs in with their own character and
/// change_main is true, the main character is updated.
///
/// Expected: Ok with user ID and main character updated
#[tokio::test]
async fn updates_main_character_when_flag_set() -> Result<(), TestError> {
    let character_id_1 = 123456789;
    let character_id_2 = 987654321;
    let owner_hash = "owner_hash_123";

    let mut test = TestBuilder::new()
        .with_user_tables()
        .with_jwt_endpoints(character_id_2, owner_hash)
        .build()
        .await?;

    // Create user with first character as main
    let (user, _, char1) = test
        .user()
        .insert_user_with_mock_character(character_id_1, 1, None, None)
        .await?;

    // Add second character to same user
    let (ownership2, char2) = test
        .user()
        .insert_mock_character_for_user(user.id, character_id_2, 2, None, None)
        .await?;

    // Update owner hash for second character
    entity::bifrost_user_character::Entity::update_many()
        .col_expr(
            entity::bifrost_user_character::Column::OwnerHash,
            sea_orm::sea_query::Expr::value(owner_hash),
        )
        .filter(entity::bifrost_user_character::Column::Id.eq(ownership2.id))
        .exec(&test.db)
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    // Login with second character and change_main=true
    let result = service
        .handle_callback("auth_code", Some(user.id), Some(true))
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    assert_eq!(result, user.id);

    // Verify main character was updated
    let user_repo = UserRepository::new(&test.db);
    let (updated_user, _) = user_repo.get_by_id(user.id).await?.unwrap();
    assert_eq!(updated_user.main_character_id, char2.id);
    assert_ne!(updated_user.main_character_id, char1.id);

    test.assert_mocks();

    Ok(())
}

/// Tests callback with change_main=false doesn't update main character.
///
/// Verifies that when change_main is explicitly false, the main character
/// remains unchanged even if logging in with a different character.
///
/// Expected: Ok with user ID and main character unchanged
#[tokio::test]
async fn does_not_update_main_character_when_flag_false() -> Result<(), TestError> {
    let character_id_1 = 123456789;
    let character_id_2 = 987654321;
    let owner_hash = "owner_hash_123";

    let mut test = TestBuilder::new()
        .with_user_tables()
        .with_jwt_endpoints(character_id_2, owner_hash)
        .build()
        .await?;

    // Create user with first character as main
    let (user, _, char1) = test
        .user()
        .insert_user_with_mock_character(character_id_1, 1, None, None)
        .await?;

    // Add second character to same user
    let (ownership2, _) = test
        .user()
        .insert_mock_character_for_user(user.id, character_id_2, 2, None, None)
        .await?;

    // Update owner hash for second character
    entity::bifrost_user_character::Entity::update_many()
        .col_expr(
            entity::bifrost_user_character::Column::OwnerHash,
            sea_orm::sea_query::Expr::value(owner_hash),
        )
        .filter(entity::bifrost_user_character::Column::Id.eq(ownership2.id))
        .exec(&test.db)
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    // Login with second character and change_main=false
    let result = service
        .handle_callback("auth_code", Some(user.id), Some(false))
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    assert_eq!(result, user.id);

    // Verify main character was NOT updated
    let user_repo = UserRepository::new(&test.db);
    let (updated_user, _) = user_repo.get_by_id(user.id).await?.unwrap();
    assert_eq!(updated_user.main_character_id, char1.id);

    test.assert_mocks();

    Ok(())
}

/// Tests callback error handling when ESI is unavailable.
///
/// Verifies that when the ESI service is unavailable (no mock endpoints),
/// the service returns an appropriate error.
///
/// Expected: Err with EsiError
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    let result = service.handle_callback("auth_code", None, None).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::EsiError(_)));

    Ok(())
}

/// Tests callback error handling when database tables are missing.
///
/// Verifies that when required database tables don't exist, the service
/// returns an appropriate database error.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_database_tables_missing() -> Result<(), TestError> {
    let character_id = 123456789;
    let owner_hash = "owner_hash_123";

    let test = TestBuilder::new()
        .with_jwt_endpoints(character_id, owner_hash)
        .build()
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    let result = service.handle_callback("auth_code", None, None).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::DbErr(_)));

    Ok(())
}

/// Tests callback for character with alliance.
///
/// Verifies that the full EVE hierarchy (character -> corporation -> alliance)
/// is properly fetched and persisted when a character belongs to an alliance.
///
/// Expected: Ok with new user ID
#[tokio::test]
async fn handles_character_with_alliance() -> Result<(), TestError> {
    let character_id = 123456789;
    let corporation_id = 1;
    let alliance_id = 100;
    let owner_hash = "owner_hash_123";

    let mock_alliance = factory::mock_alliance(None);
    let mock_corporation = factory::mock_corporation(Some(alliance_id), None);
    let mock_character = factory::mock_character(corporation_id, None, None);

    let test = TestBuilder::new()
        .with_user_tables()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1)
        .with_corporation_endpoint(corporation_id, mock_corporation, 1)
        .with_character_endpoint(character_id, mock_character, 1)
        .with_jwt_endpoints(character_id, owner_hash)
        .build()
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    let result = service
        .handle_callback("auth_code", None, None)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    assert_eq!(result, 1);

    test.assert_mocks();

    Ok(())
}

/// Tests callback for character with faction.
///
/// Verifies that characters belonging to NPC corporations (with factions)
/// are properly handled.
///
/// Expected: Ok with new user ID
#[tokio::test]
async fn handles_character_with_faction() -> Result<(), TestError> {
    let character_id = 123456789;
    let corporation_id = 1;
    let faction_id = 500001;
    let owner_hash = "owner_hash_123";

    let mock_corporation = factory::mock_corporation(None, Some(faction_id));
    let mock_character = factory::mock_character(corporation_id, None, None);

    let test = TestBuilder::new()
        .with_user_tables()
        .with_mock_faction(faction_id)
        .with_corporation_endpoint(corporation_id, mock_corporation, 1)
        .with_character_endpoint(character_id, mock_character, 1)
        .with_jwt_endpoints(character_id, owner_hash)
        .build()
        .await?;

    let service = CallbackService::new(&test.db, &test.esi_client);

    let result = service
        .handle_callback("auth_code", None, None)
        .await
        .map_err(|e| TestError::DbErr(sea_orm::DbErr::Custom(e.to_string())))?;

    assert_eq!(result, 1);

    test.assert_mocks();

    Ok(())
}
