use axum::{extract::State, http::StatusCode, response::IntoResponse};
use bifrost::server::controller::auth::login;

use crate::util::setup::{test_setup, TEST_USER_AGENT};

#[tokio::test]
// Test the return of a 307 temporary redirect response for login
async fn test_login_success() {
    let test = test_setup().await;

    let result = login(State(test.state), test.session).await;

    assert!(result.is_ok());

    let resp = result.unwrap().into_response();

    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT)
}

#[tokio::test]
// Test the return of a 500 internal server error response for failed login
async fn test_login_server_error() {
    let mut test = test_setup().await;

    // Build an ESI client not configured for OAuth2 to trigger internal server error
    let esi_client = eve_esi::Client::new(TEST_USER_AGENT).unwrap();
    test.state.esi_client = esi_client;

    let result = login(State(test.state), test.session).await;

    assert!(result.is_err());

    let resp = result.err().unwrap().into_response();

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR)
}
