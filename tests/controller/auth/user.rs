//! Tests for the get_user endpoint.
//!
//! This module verifies the get_user endpoint's behavior, including successful
//! retrieval of user information for authenticated users, 404 responses for
//! missing or non-existent users, and error handling for database issues.

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use bifrost::server::{controller::auth::get_user, model::session::user::SessionUserId};

use super::*;

/// Tests successful retrieval of user information for logged-in user.
///
/// Verifies that the get_user endpoint returns a 200 OK response with user
/// information when a valid user ID exists in the session and the corresponding
/// user is found in the database.
///
/// Expected: Ok with 200 OK response
#[tokio::test]
async fn found_for_logged_in_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    SessionUserId::insert(&test.session, user_model.id)
        .await
        .unwrap();

    let result = get_user(State(test.into_app_state()), test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}

/// Tests 404 response when no user is logged in.
///
/// Verifies that the get_user endpoint returns a 404 NOT FOUND response when
/// there is no user ID in the session, indicating no authenticated user.
///
/// Expected: Err with 404 NOT_FOUND response
#[tokio::test]
async fn not_found_for_user_not_logged_in() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let result = get_user(State(test.into_app_state()), test.session).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    Ok(())
}

/// Tests 404 response when session user is not in database.
///
/// Verifies that the get_user endpoint returns a 404 NOT FOUND response when
/// the user ID from the session doesn't correspond to any user record in the
/// database, handling stale session data gracefully.
///
/// Expected: Err with 404 NOT_FOUND response
#[tokio::test]
async fn not_found_for_user_not_in_database() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    // Set a user ID in session but don't put them in database
    let non_existant_user_id = 1;
    SessionUserId::insert(&test.session, non_existant_user_id)
        .await
        .unwrap();

    let result = get_user(State(test.into_app_state()), test.session).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the get_user endpoint returns a 500 INTERNAL SERVER ERROR
/// response when required database tables don't exist, indicating a critical
/// infrastructure issue rather than a user error.
///
/// Expected: Err with 500 INTERNAL_SERVER_ERROR response
#[tokio::test]
async fn error_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    // Set user in session so that database is checked for user
    let non_existant_user_id = 1;
    SessionUserId::insert(&test.session, non_existant_user_id)
        .await
        .unwrap();

    let result = get_user(State(test.into_app_state()), test.session).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}
