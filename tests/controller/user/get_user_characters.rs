use axum::{extract::State, http::StatusCode, response::IntoResponse};
use bifrost::server::{controller::user::get_user_characters, model::session::user::SessionUserId};

use super::*;

#[tokio::test]
/// Expect 200 success with empty list for user with no characters
async fn success_with_empty_list_for_user_with_no_additional_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    SessionUserId::insert(&test.session, user_model.id)
        .await
        .unwrap();

    let result = get_user_characters(State(test.into_app_state()), test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
/// Expect 200 success with single character for user with one character
async fn success_with_single_character() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    SessionUserId::insert(&test.session, user_model.id)
        .await
        .unwrap();

    let result = get_user_characters(State(test.into_app_state()), test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
/// Expect 200 success with multiple characters for user with multiple characters
async fn success_with_multiple_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Insert multiple additional characters for the user
    let (_, _) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 1, None, None)
        .await?;

    let (_, _) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 3, 2, None, None)
        .await?;

    let (_, _) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 4, 2, Some(1), None)
        .await?;

    SessionUserId::insert(&test.session, user_model.id)
        .await
        .unwrap();

    let result = get_user_characters(State(test.into_app_state()), test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
/// Expect 200 success with characters that have alliance and faction
async fn success_with_characters_having_alliance_and_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, Some(1), Some(1))
        .await?;

    // Insert character with alliance
    let (_, _) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 2, Some(2), None)
        .await?;

    // Insert character with faction
    let (_, _) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 3, 3, None, Some(2))
        .await?;

    SessionUserId::insert(&test.session, user_model.id)
        .await
        .unwrap();

    let result = get_user_characters(State(test.into_app_state()), test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
/// Expect 404 not found when user is not logged in
async fn not_found_when_user_not_logged_in() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;

    let result = get_user_characters(State(test.into_app_state()), test.session).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
/// Expect 404 not found when user in session doesn't exist in database
async fn not_found_when_user_not_in_database() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;

    // Set a user ID in session but don't put them in database
    let non_existent_user_id = 999;
    SessionUserId::insert(&test.session, non_existent_user_id)
        .await
        .unwrap();

    let result = get_user_characters(State(test.into_app_state()), test.session.clone()).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Verify session was cleared
    let session_user_id = SessionUserId::get(&test.session).await;
    assert!(session_user_id.is_ok());
    assert!(session_user_id.unwrap().is_none());

    Ok(())
}

#[tokio::test]
/// Expect 500 internal server error when required database tables don't exist
async fn error_when_tables_missing() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;

    // Set user in session so that database is checked for user
    let non_existent_user_id = 1;
    SessionUserId::insert(&test.session, non_existent_user_id)
        .await
        .unwrap();

    let result = get_user_characters(State(test.into_app_state()), test.session).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

#[tokio::test]
/// Expect 200 success and verify only characters for specific user are returned
async fn returns_only_characters_for_logged_in_user() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;

    // Create first user with characters
    let (user_model_1, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (_, _) = test
        .user()
        .insert_mock_character_for_user(user_model_1.id, 2, 1, None, None)
        .await?;

    // Create second user with characters
    let (user_model_2, _, _) = test
        .user()
        .insert_user_with_mock_character(3, 1, None, None)
        .await?;
    let (_, _) = test
        .user()
        .insert_mock_character_for_user(user_model_2.id, 4, 1, None, None)
        .await?;

    // Login as first user
    SessionUserId::insert(&test.session, user_model_1.id)
        .await
        .unwrap();

    let result = get_user_characters(State(test.into_app_state()), test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}
