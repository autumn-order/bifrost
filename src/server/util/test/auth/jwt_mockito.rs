use eve_esi::model::oauth2::EveJwtClaims;
use mockito::{Mock, ServerGuard};

use crate::server::util::test::auth::jwt::{create_mock_jwt_keys, create_mock_jwt_token};

/// Provides mock endpoints for JWT token & keys used for callback after successful login
pub fn mock_jwt_endpoints(server: &mut ServerGuard) -> (Mock, Mock) {
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
