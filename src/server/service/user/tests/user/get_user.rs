use crate::server::{error::Error, service::user::UserService};

use super::*;

/// Expect Ok with Some & no additional characters for user with only a main character linked
#[tokio::test]
async fn returns_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let user_service = UserService::new(&test.state.db);
    let result = user_service.get_user(user_model.id).await;
    assert!(result.is_ok());
    let maybe_user = result.unwrap();
    assert!(maybe_user.is_some());

    Ok(())
}

/// Expect Ok with None for user ID that does not exist
#[tokio::test]
async fn returns_none_for_nonexistent_user() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let nonexistent_user_id = 1;
    let user_service = UserService::new(&test.state.db);
    let result = user_service.get_user(nonexistent_user_id).await;

    assert!(result.is_ok());
    let maybe_user = result.unwrap();
    assert!(maybe_user.is_none());

    Ok(())
}

/// Expect Error when required tables are not present
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let nonexistent_user_id = 1;
    let user_service = UserService::new(&test.state.db);
    let result = user_service.get_user(nonexistent_user_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}
