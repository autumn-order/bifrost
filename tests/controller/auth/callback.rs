use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use bifrost::server::{
    controller::auth::callback::{callback, CallbackParams},
    model::session::AuthLoginCsrf,
};
use eve_esi::model::oauth2::EveJwtClaims;
use mockito::{Mock, ServerGuard};

use crate::{
    setup::{test_setup, TestSetup},
    util::auth::jwt::{create_mock_jwt_keys, create_mock_jwt_token},
};

async fn setup() -> (TestSetup, CallbackParams) {
    let test = test_setup().await;

    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };

    // Insert CSRF state into session for CSRF validation in callback
    AuthLoginCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();

    (test, params)
}

/// Provides mock endpoints for JWT token & keys used for callback after successful login
fn mock_jwt_endpoints(server: &mut ServerGuard) -> (Mock, Mock) {
    let mock_keys = create_mock_jwt_keys();

    let claims = EveJwtClaims::mock();
    let mock_token = create_mock_jwt_token(claims);

    let mock_jwt_key_endpoint = server
        .mock("GET", "/oauth/jwks")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_keys).unwrap())
        .create();

    let mock_jwt_token_endpoint = server
        .mock("POST", "/v2/oauth/token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_token).unwrap())
        .create();

    (mock_jwt_key_endpoint, mock_jwt_token_endpoint)
}

#[tokio::test]
// TODO: This test does not yet pass, it needs to implement mock server endpoints for JWT keys and JWT tokens
// for token validation to ensure complete testing of the callback controller.
//
// Test the return of a 200 success response for callback
async fn test_callback_success() {
    let (mut test, params) = setup().await;
    let (mock_jwt_key_endpoint, mock_jwt_token_endpoint) = mock_jwt_endpoints(&mut test.server);

    let result = callback(State(test.state), test.session, Query(params)).await;

    // Assert JWT keys & token were fetched during callback
    mock_jwt_key_endpoint.assert();
    mock_jwt_token_endpoint.assert();

    assert!(result.is_ok());

    let resp = result.unwrap().into_response();

    assert_eq!(resp.status(), StatusCode::OK)
}

#[tokio::test]
// Test the return of a 400 bad request error response for callback
async fn test_callback_bad_request() {
    let (test, mut params) = setup().await;

    // Modify params CSRF state to trigger server error for failing CSRF validation
    params.state = "incorrect_state".to_string();

    let result = callback(State(test.state), test.session, Query(params)).await;

    assert!(result.is_err());

    let resp = result.err().unwrap().into_response();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST)
}

#[tokio::test]
// Test the return of a 500 internal server error response for callback
async fn test_callback_server_error() {
    let (test, params) = setup().await;

    // Don't create any mock JWT token or key endpoints so that token validation fails

    let result = callback(State(test.state), test.session, Query(params)).await;

    assert!(result.is_err());

    let resp = result.err().unwrap().into_response();

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR)
}
