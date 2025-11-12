use crate::server::data::user::UserRepository;

use super::*;

/// Expect Ok with user deletion when last character is transferred
#[tokio::test]
async fn deletes_user_when_last_character_transferred() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (_, user_character_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    // Character is set as main but there isn't actually an ownership record set so it will transfer
    let new_user_model = test.user().insert_user(character_model.id).await?;

    let user_character_repo = UserCharacterRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .transfer_character(user_character_model, new_user_model.id)
        .await;

    assert!(result.is_ok());
    let previous_user_deleted = result.unwrap();
    assert!(previous_user_deleted);

    // Ensure character was actually transferred
    let ownership_entry = user_character_repo
        .get_by_character_id(character_model.character_id)
        .await?;
    let (_, maybe_ownership) = ownership_entry.unwrap();
    let character_ownership = maybe_ownership.unwrap();
    assert_eq!(character_ownership.user_id, new_user_model.id);

    Ok(())
}

/// Expect Ok with no user deletion when character is transferred from user with multiple characters
/// - No main change
#[tokio::test]
async fn transfers_character_without_deleting_user() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (second_user_character_model, character_model) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 1, None, None)
        .await?;
    // Character is set as main but there isn't actually an ownership record set so it will transfer
    let new_user_model = test.user().insert_user(character_model.id).await?;

    let user_repo = UserRepository::new(&test.state.db);
    let user_character_repo = UserCharacterRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .transfer_character(second_user_character_model, new_user_model.id)
        .await;

    assert!(result.is_ok());
    let previous_user_deleted = result.unwrap();
    assert!(!previous_user_deleted);

    // Ensure character was actually transferred
    let ownership_entry = user_character_repo
        .get_by_character_id(character_model.character_id)
        .await?;
    let (_, maybe_ownership) = ownership_entry.unwrap();
    let character_ownership = maybe_ownership.unwrap();
    assert_eq!(character_ownership.user_id, new_user_model.id);

    // Ensure main character was not changed since it wasn't transferred
    let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
    assert_eq!(
        user_model.main_character_id,
        updated_user_model.main_character_id
    );

    Ok(())
}

/// Expect Ok with no user deletion when character is transferred from user with multiple characters
/// - change main
#[tokio::test]
async fn changes_main_character_after_transfer() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, main_user_character_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (_, _) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 1, None, None)
        .await?;
    // Character is set as main but there isn't actually an ownership record set so it will transfer
    let new_user_model = test.user().insert_user(character_model.id).await?;

    let user_repo = UserRepository::new(&test.state.db);
    let user_character_repo = UserCharacterRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .transfer_character(main_user_character_model, new_user_model.id)
        .await;

    assert!(result.is_ok());
    let previous_user_deleted = result.unwrap();
    assert!(!previous_user_deleted);

    // Ensure character was actually transferred
    let ownership_entry = user_character_repo
        .get_by_character_id(character_model.character_id)
        .await?;
    let (_, maybe_ownership) = ownership_entry.unwrap();
    let character_ownership = maybe_ownership.unwrap();
    assert_eq!(character_ownership.user_id, new_user_model.id);

    // Ensure main character was changed since it was transferred
    let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
    assert_ne!(
        user_model.main_character_id,
        updated_user_model.main_character_id
    );

    Ok(())
}

/// Expect Error transferring character to user that does not exist
#[tokio::test]
async fn fails_for_nonexistent_target_user() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, user_character_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let user_character_repo = UserCharacterRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .transfer_character(user_character_model.clone(), user_model.id + 1)
        .await;

    assert!(result.is_err());

    // Ensure character was not transferred
    let ownership_entry = user_character_repo
        .get_by_character_id(character_model.character_id)
        .await?;
    let (_, maybe_ownership) = ownership_entry.unwrap();
    let latest_character_ownership = maybe_ownership.unwrap();
    assert_eq!(
        latest_character_ownership.user_id,
        user_character_model.user_id
    );

    Ok(())
}
