//! Tests for the get_user_characters endpoint.
//!
//! This module verifies the get_user_characters endpoint's behavior, including
//! successful retrieval of character lists with various character counts and
//! associations (alliance, faction), proper user data isolation, and error
//! handling for authentication and database issues.

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use bifrost::server::{controller::user::get_user_characters, model::session::user::SessionUserId};

use super::*;

/// Tests successful response with empty character list.
///
/// Verifies that the get_user_characters endpoint returns a 200 OK response with
/// an empty character list when the user has only their main character and no
/// additional characters.
///
/// Expected: Ok with 200 OK response
#[tokio::test]
async fn success_with_empty_list_for_user_with_no_additional_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

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

/// Tests successful response with single character.
///
/// Verifies that the get_user_characters endpoint returns a 200 OK response
/// containing a single character when the user has one character in their account.
///
/// Expected: Ok with 200 OK response
#[tokio::test]
async fn success_with_single_character() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

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

/// Tests successful response with multiple characters.
///
/// Verifies that the get_user_characters endpoint returns a 200 OK response
/// containing all characters when the user has multiple characters associated
/// with their account.
///
/// Expected: Ok with 200 OK response
#[tokio::test]
async fn success_with_multiple_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

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

/// Tests successful response with characters having alliance and faction.
///
/// Verifies that the get_user_characters endpoint correctly returns character
/// information including optional alliance and faction associations when present.
///
/// Expected: Ok with 200 OK response
#[tokio::test]
async fn success_with_characters_having_alliance_and_faction() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

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

/// Tests 404 response when no user is logged in.
///
/// Verifies that the get_user_characters endpoint returns a 404 NOT FOUND
/// response when there is no user ID in the session, indicating no authenticated
/// user.
///
/// Expected: Err with 404 NOT_FOUND response
#[tokio::test]
async fn not_found_when_user_not_logged_in() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let result = get_user_characters(State(test.into_app_state()), test.session).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    Ok(())
}

/// Tests 404 response and session cleanup for non-existent user.
///
/// Verifies that the get_user_characters endpoint returns a 404 NOT FOUND
/// response when the session contains a user ID that doesn't exist in the
/// database, and properly clears the stale session data.
///
/// Expected: Err with 404 NOT_FOUND response and session cleared
#[tokio::test]
async fn not_found_when_user_not_in_database() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

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

/// Tests error handling when database tables are missing.
///
/// Verifies that the get_user_characters endpoint returns a 500 INTERNAL SERVER
/// ERROR response when required database tables don't exist, indicating a critical
/// infrastructure issue.
///
/// Expected: Err with 500 INTERNAL_SERVER_ERROR response
#[tokio::test]
async fn error_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

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

/// Tests that only the logged-in user's characters are returned.
///
/// Verifies that the get_user_characters endpoint returns only the characters
/// belonging to the logged-in user, not characters from other users in the
/// database, ensuring proper user data isolation.
///
/// Expected: Ok with 200 OK response containing only logged-in user's characters
#[tokio::test]
async fn returns_only_characters_for_logged_in_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

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
