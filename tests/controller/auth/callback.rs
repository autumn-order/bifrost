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
use sea_orm::{ConnectionTrait, DbBackend, DbErr, Schema};

use crate::util::{
    auth::jwt::{create_mock_jwt_keys, create_mock_jwt_token},
    mock::{mock_character, mock_corporation},
    mockito::{character::mock_character_endpoint, corporation::mock_corporation_endpoint},
    setup::{test_setup, TestSetup},
};

async fn setup() -> Result<(TestSetup, CallbackParams), DbErr> {
    let test = test_setup().await;

    let db = &test.state.db;
    let schema = Schema::new(DbBackend::Sqlite);

    let stmts = vec![
        schema.create_table_from_entity(entity::prelude::EveFaction),
        schema.create_table_from_entity(entity::prelude::EveAlliance),
        schema.create_table_from_entity(entity::prelude::EveCorporation),
        schema.create_table_from_entity(entity::prelude::EveCharacter),
    ];

    for stmt in stmts {
        db.execute(&stmt).await?;
    }

    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };

    // Insert CSRF state into session for CSRF validation in callback
    AuthLoginCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();

    Ok((test, params))
}

/// Provides mock endpoints for JWT token & keys used for callback after successful login
fn mock_jwt_endpoints(server: &mut ServerGuard) -> (Mock, Mock) {
    let mock_keys = create_mock_jwt_keys();

    let mut claims = EveJwtClaims::mock();
    // Set character ID to 1 which is the default used for mock_character used across tests
    claims.sub = "CHARACTER:EVE:1".to_string();

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
async fn test_callback_success() -> Result<(), DbErr> {
    let (mut test, params) = setup().await?;
    let (mock_jwt_key_endpoint, mock_jwt_token_endpoint) = mock_jwt_endpoints(&mut test.server);

    // Create the mock character & corporation that will be fetched during callback
    let alliance_id = None;
    let faction_id = None;
    let mock_corporation = mock_corporation(alliance_id, faction_id);

    let corporation_id = 1;
    let mock_character = mock_character(corporation_id, alliance_id, faction_id);

    let expected_requests = 1;
    let corporation_endpoint = mock_corporation_endpoint(
        &mut test.server,
        "/corporations/1",
        mock_corporation,
        expected_requests,
    );
    let character_endpoint = mock_character_endpoint(
        &mut test.server,
        "/characters/1",
        mock_character,
        expected_requests,
    );

    let result = callback(State(test.state), test.session, Query(params)).await;

    assert!(result.is_ok());

    // Assert JWT keys & token were fetched during callback
    mock_jwt_key_endpoint.assert();
    mock_jwt_token_endpoint.assert();

    // Assert character endpoints were fetched during callback when creating character entry
    character_endpoint.assert();
    corporation_endpoint.assert();

    let resp = result.unwrap().into_response();

    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
// Test the return of a 400 bad request error response for callback
async fn test_callback_bad_request() -> Result<(), DbErr> {
    let (test, mut params) = setup().await?;

    // Modify params CSRF state to trigger server error for failing CSRF validation
    params.state = "incorrect_state".to_string();

    let result = callback(State(test.state), test.session, Query(params)).await;

    assert!(result.is_err());

    let resp = result.err().unwrap().into_response();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
// Test the return of a 500 internal server error response for callback
async fn test_callback_server_error() -> Result<(), DbErr> {
    let (test, params) = setup().await?;

    // Don't create any mock JWT token or key endpoints so that token validation fails

    let result = callback(State(test.state), test.session, Query(params)).await;

    assert!(result.is_err());

    let resp = result.err().unwrap().into_response();

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}
