use crate::server::{error::Error, service::user::UserService};

use super::*;

/// Expect Ok with true indicating user was deleted
#[tokio::test]
async fn deletes_user_successfully() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    // We include the character ID as a main which must be set for every user, for this test
    // they don't actually need to own the character so no ownership record is set.
    let user_model = test.user().insert_user(character_model.id).await?;

    let user_service = UserService::new(&test.state.db, &test.state.esi_client);
    let result = user_service.delete_user(user_model.id).await;

    assert!(result.is_ok());
    let user_deleted = result.unwrap();
    assert!(user_deleted);
    let maybe_user = user_service.get_user(user_model.id).await.unwrap();
    assert!(maybe_user.is_none());

    Ok(())
}

/// Expect Ok with false when trying to delete a user that does not exist
#[tokio::test]
async fn returns_false_for_nonexistent_user() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;
    let nonexistent_user_id = 1;
    let user_service = UserService::new(&test.state.db, &test.state.esi_client);
    let result = user_service.delete_user(nonexistent_user_id).await;

    assert!(result.is_ok());
    let user_deleted = result.unwrap();
    assert!(!user_deleted);

    Ok(())
}

/// Expect Error when trying to delete user with existing character ownerships
/// - This is due to a foreign key violation requiring a user ID to exist for
///   a character ownership entry.
#[tokio::test]
async fn fails_when_user_has_owned_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let user_service = UserService::new(&test.state.db, &test.state.esi_client);
    let result = user_service.delete_user(user_model.id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}
