use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use bifrost::server::{
    controller::auth::callback::{callback, CallbackParams},
    model::session::AuthLoginCsrf,
};

use crate::setup::{test_setup, TestSetup};

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

#[tokio::test]
// TODO: This test does not yet pass, it needs to implement mock server endpoints for JWT keys and JWT tokens
// for token validation to ensure complete testing of the callback controller.
//
// Test the return of a 200 success response for callback
async fn test_callback_success() {
    let (test, params) = setup().await;

    // Create mock JWT key endpoint
    //
    // Create mock

    let result = callback(State(test.state), test.session, Query(params)).await;

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

    let result = callback(State(test.state), test.session, Query(params)).await;

    assert!(result.is_err());

    let resp = result.err().unwrap().into_response();

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR)
}
