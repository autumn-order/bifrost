use axum::{extract::State, http::StatusCode, response::IntoResponse};
use bifrost::server::{controller::auth::get_user, model::session::user::SessionUserId};

use super::*;

#[tokio::test]
/// Expect 200 success with user information for existing user
async fn found_for_logged_in_user() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    SessionUserId::insert(&test.session, user_model.id)
        .await
        .unwrap();

    let result = get_user(State(test.state()), test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
/// Expect 404 not found for user that isn't in session
async fn not_found_for_user_not_logged_in() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;

    let result = get_user(State(test.state()), test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
/// Expect 404 not found for user that isn't in database
async fn not_found_for_user_not_in_database() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;

    // Set a user ID in session but don't put them in database
    let non_existant_user_id = 1;
    SessionUserId::insert(&test.session, non_existant_user_id)
        .await
        .unwrap();

    let result = get_user(State(test.state()), test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
/// Expect 500 internal server error when required database tables dont exist
async fn error_when_tables_missing() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;

    // Set user in session so that database is checked for user
    let non_existant_user_id = 1;
    SessionUserId::insert(&test.session, non_existant_user_id)
        .await
        .unwrap();

    let result = get_user(State(test.state()), test.session).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}
