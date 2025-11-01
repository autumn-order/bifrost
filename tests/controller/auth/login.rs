use axum::{extract::State, http::StatusCode, response::IntoResponse};
use bifrost::server::controller::auth::login;
use bifrost_test_utils::{constant::TEST_USER_AGENT, prelude::*};

#[tokio::test]
// Test the return of a 307 temporary redirect response for login
async fn redirects_to_eve_login() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;

    let result = login(State(test.state()), test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);

    Ok(())
}

#[tokio::test]
// Test the return of a 500 internal server error response for failed login
async fn fails_when_oauth2_not_configured() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!()?;
    // Build an ESI client not configured for OAuth2 to trigger internal server error
    let esi_client = eve_esi::Client::new(TEST_USER_AGENT).unwrap();
    test.state.esi_client = esi_client;

    let result = login(State(test.state()), test.session).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}
