use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use bifrost::server::{
    controller::auth::{callback, CallbackParams},
    model::session::{auth::SessionAuthCsrf, user::SessionUserId},
};
use bifrost_test_utils::prelude::*;

#[tokio::test]
/// Expect 307 temprorary redirect when logging with new character
async fn test_callback_new_user_success() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let character_id = 1;
    let character_endpoints = test
        .eve()
        .with_character_endpoint(character_id, 1, None, None, 1);
    let jwt_endpoints = test.auth().with_jwt_endpoints(character_id, "owner_hash");
    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };
    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();

    let result = callback(State(test.state()), test.session.clone(), Query(params)).await;

    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);

    // Assert user is in session
    let result = SessionUserId::get(&test.session).await;
    assert!(result.is_ok());
    let maybe_user_id = result.unwrap();
    assert!(maybe_user_id.is_some());
    let user_id = maybe_user_id.unwrap();

    // User ID should be 1 as it would be the first user created in database
    assert_eq!(user_id, 1);

    // Assert JWT keys & token were fetched during callback
    for endpoint in jwt_endpoints {
        endpoint.assert();
    }

    // Assert character endpoints were fetched during callback when creating character entry
    for endpoint in character_endpoints {
        endpoint.assert();
    }

    Ok(())
}

#[tokio::test]
/// Expect 307 temprorary redirect when logging with existing user
async fn test_callback_existing_user_success() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, user_character_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let jwt_endpoints = test.auth().with_jwt_endpoints(
        character_model.character_id,
        &user_character_model.owner_hash,
    );
    SessionUserId::insert(&test.session, user_model.id)
        .await
        .unwrap();
    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };

    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();
    let result = callback(State(test.state()), test.session.clone(), Query(params)).await;

    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);

    // Assert user is in session
    let result = SessionUserId::get(&test.session).await;
    assert!(result.is_ok());
    let maybe_user_id = result.unwrap();
    assert!(maybe_user_id.is_some());
    let user_id = maybe_user_id.unwrap();

    // User ID should be 1 as it would be the first user created in database
    assert_eq!(user_id, 1);

    // Assert JWT keys & token were fetched during callback
    for endpoint in jwt_endpoints {
        endpoint.assert();
    }

    Ok(())
}

#[tokio::test]
/// Expect 400 bad request when CSRF state is modified
async fn test_callback_bad_request() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;
    let mut params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };
    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();
    params.state = "incorrect_state".to_string();

    let result = callback(State(test.state()), test.session, Query(params)).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
/// Test the return of a 500 internal server error response for callback
async fn test_callback_server_error() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;
    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };

    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();
    let result = callback(State(test.state()), test.session, Query(params)).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}
