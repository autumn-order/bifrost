use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use bifrost::server::controller::user::get_user;
use bifrost_test_utils::prelude::*;

#[tokio::test]
/// Expect 200 success with user information for existing user
async fn returns_success_for_existing_user() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let result = get_user(State(test.state()), Path(user_model.id)).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
/// Expect 404 not found for user that does not exist
async fn returns_not_found_for_user_that_doesnt_exist() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;

    let non_existant_user_id = 1;
    let result = get_user(State(test.state()), Path(non_existant_user_id)).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
/// Expect 500 internal server error when required database tables dont exist
async fn error_when_required_tables_dont_exist() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;

    let non_existant_user_id = 1;
    let result = get_user(State(test.state()), Path(non_existant_user_id)).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}
