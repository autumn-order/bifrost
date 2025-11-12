use crate::server::{
    data::user::UserRepository, service::user::user_character::UserCharacterService,
};

use super::*;

/// Expect Ok when changing main character to another owned character
#[tokio::test]
async fn changes_main_to_owned_character() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (_, second_character_model) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 1, None, None)
        .await?;

    let user_repo = UserRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .change_main(user_model.id, second_character_model.character_id)
        .await;

    assert!(result.is_ok());

    // Verify main character was actually changed
    let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
    assert_eq!(
        updated_user_model.main_character_id,
        second_character_model.id
    );
    assert_ne!(
        user_model.main_character_id,
        updated_user_model.main_character_id
    );

    Ok(())
}

/// Expect Ok but no change when attempting to change main to current main character
#[tokio::test]
async fn handles_changing_to_current_main() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let user_repo = UserRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .change_main(user_model.id, character_model.character_id)
        .await;

    assert!(result.is_ok());

    // Verify main character remains unchanged
    let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
    assert_eq!(
        updated_user_model.main_character_id,
        user_model.main_character_id
    );

    Ok(())
}

/// Expect Ok (with warning logged) when attempting to change main to unowned character
#[tokio::test]
async fn handles_unowned_character_gracefully() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    // Create a character that exists but is not owned by this user
    let unowned_character = test.eve().insert_mock_character(2, 1, None, None).await?;

    let user_repo = UserRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .change_main(user_model.id, unowned_character.character_id)
        .await;

    // Should return Ok() but not actually change the main
    assert!(result.is_ok());

    // Verify main character was NOT changed
    let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
    assert_eq!(
        updated_user_model.main_character_id,
        user_model.main_character_id
    );

    Ok(())
}

/// Expect Ok (with error logged) when attempting to change main to nonexistent character
#[tokio::test]
async fn handles_nonexistent_character_gracefully() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let nonexistent_character_id = 999;
    let user_repo = UserRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .change_main(user_model.id, nonexistent_character_id)
        .await;

    // Should return Ok() but not actually change the main
    assert!(result.is_ok());

    // Verify main character was NOT changed
    let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
    assert_eq!(
        updated_user_model.main_character_id,
        user_model.main_character_id
    );

    Ok(())
}

/// Expect Ok with no main change when attempting to change to a character owned by a different user
///
/// This tests that the ownership verification is working correctly within the `change_main` method.
///
/// # Important Context
/// In a fully integrated system, if a user logs in with a different character, that character
/// would first be transferred to the current user via `transfer_character` before `change_main`
/// is called. This test ensures `change_main` itself has proper ownership validation as a
/// defensive measure, preventing misuse if the method is called improperly or integration
/// logic changes in the future
#[tokio::test]
async fn prevents_changing_to_character_owned_by_different_user() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    // Create a second user with their own character
    let (_, _, other_user_character) = test
        .user()
        .insert_user_with_mock_character(2, 1, None, None)
        .await?;

    let user_repo = UserRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .change_main(user_model.id, other_user_character.character_id)
        .await;

    // Should return Ok() but not actually change the main due to ownership check
    assert!(result.is_ok());

    // Verify main character was NOT changed
    let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
    assert_eq!(
        updated_user_model.main_character_id,
        user_model.main_character_id
    );

    Ok(())
}

/// Expect Ok when user has multiple characters and changes between them
#[tokio::test]
async fn changes_main_among_multiple_owned_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (_, second_character) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 1, None, None)
        .await?;
    let (_, third_character) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 3, 1, None, None)
        .await?;

    let user_repo = UserRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);

    // Change to second character
    let result = user_character_service
        .change_main(user_model.id, second_character.character_id)
        .await;
    assert!(result.is_ok());

    let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
    assert_eq!(updated_user_model.main_character_id, second_character.id);

    // Change to third character
    let result = user_character_service
        .change_main(user_model.id, third_character.character_id)
        .await;
    assert!(result.is_ok());

    let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
    assert_eq!(updated_user_model.main_character_id, third_character.id);

    Ok(())
}
