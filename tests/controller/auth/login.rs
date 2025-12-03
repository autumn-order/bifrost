//! Tests for the login endpoint.
//!
//! This module verifies the login endpoint's behavior, including successful
//! redirect to EVE Online SSO for authentication and error handling when
//! OAuth2 configuration is missing or invalid.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use bifrost::server::controller::auth::{login, LoginParams};
use bifrost_test_utils::constant::TEST_USER_AGENT;

use super::*;

/// Tests successful redirect to EVE Online login page.
///
/// Verifies that the login endpoint returns a 307 temporary redirect response
/// that directs the user to the EVE Online SSO login page for authentication.
///
/// Expected: Ok with 307 TEMPORARY_REDIRECT response
#[tokio::test]
async fn redirects_to_eve_login() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let params = LoginParams { change_main: None };
    let result = login(State(test.into_app_state()), test.session, Query(params)).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);

    Ok(())
}

/// Tests error handling when OAuth2 is not configured.
///
/// Verifies that the login endpoint returns a 500 internal server error when
/// the ESI client is not properly configured with OAuth2 credentials, preventing
/// the redirect to EVE Online SSO.
///
/// Expected: Err with 500 INTERNAL_SERVER_ERROR response
#[tokio::test]
async fn fails_when_oauth2_not_configured() -> Result<(), TestError> {
    let mut test = TestBuilder::new().build().await?;
    // Build an ESI client not configured for OAuth2 to trigger internal server error
    let esi_client = eve_esi::Client::new(TEST_USER_AGENT).unwrap();
    test.esi_client = esi_client;

    let params = LoginParams { change_main: None };
    let result = login(State(test.into_app_state()), test.session, Query(params)).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}
