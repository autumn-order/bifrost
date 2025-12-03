use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use bifrost::server::{
    controller::auth::{callback, CallbackParams},
    model::session::{auth::SessionAuthCsrf, user::SessionUserId},
};

use super::*;

#[tokio::test]
/// Expect 307 temprorary redirect when logging with new character
async fn redirects_for_new_user() -> Result<(), TestError> {
    let corporation_id = 1;
    let character_id = 1;
    let mock_corporation = factory::mock_corporation(None, None);
    let mock_character = factory::mock_character(corporation_id, None, None);

    let test = TestBuilder::new()
        .with_user_tables()
        .with_corporation_endpoint(corporation_id, mock_corporation, 1)
        .with_character_endpoint(character_id, mock_character, 1)
        .with_jwt_endpoints(character_id, "owner_hash")
        .build()
        .await?;

    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };
    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();

    let result = callback(
        State(test.into_app_state()),
        test.session.clone(),
        Query(params),
    )
    .await;

    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);

    // Assert user is in session
    let result = SessionUserId::get(&test.session).await;
    assert!(result.is_ok());
    let maybe_user_id = result.unwrap();
    assert!(maybe_user_id.is_some());
    let user_id = maybe_user_id.unwrap();

    // User ID should be 1 as it would be the first user created in database
    assert_eq!(user_id, 1);

    test.assert_mocks();

    Ok(())
}

#[tokio::test]
/// Expect 307 temprorary redirect when logging with existing user
async fn redirects_for_existing_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

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
    let result = callback(
        State(test.into_app_state()),
        test.session.clone(),
        Query(params),
    )
    .await;

    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);

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
async fn fails_for_invalid_csrf_state() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let mut params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };
    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();
    params.state = "incorrect_state".to_string();

    let result = callback(State(test.into_app_state()), test.session, Query(params)).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
/// Test the return of a 500 internal server error response for callback
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };

    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();
    let result = callback(State(test.into_app_state()), test.session, Query(params)).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}
