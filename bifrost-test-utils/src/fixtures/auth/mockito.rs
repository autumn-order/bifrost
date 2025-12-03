//! JWT OAuth2 HTTP mock endpoint creation utilities.
//!
//! This module provides methods for creating mock HTTP endpoints that simulate
//! EVE SSO OAuth2 authentication endpoints, including JWKS (JWT key set) and
//! token exchange endpoints.

use mockito::Mock;

use crate::fixtures::auth::AuthFixtures;

impl<'a> AuthFixtures<'a> {
    /// Create mock HTTP endpoints for JWT authentication flow.
    ///
    /// Sets up two mock endpoints required for EVE SSO OAuth2 authentication:
    /// 1. GET `/oauth/jwks` - Returns JWT public keys for signature verification
    /// 2. POST `/v2/oauth/token` - Returns signed JWT access token for authorization code
    ///
    /// These endpoints simulate a complete OAuth2 token exchange and validation flow.
    ///
    /// # Arguments
    /// - `character_id` - The EVE Online character ID to include in JWT claims
    /// - `ownerhash` - The owner hash to include in JWT claims for ownership verification
    ///
    /// # Returns
    /// - `Vec<Mock>` - Vector containing both mock endpoints (JWKS and token) for verification
    pub fn create_jwt_endpoints(&mut self, character_id: i64, ownerhash: &str) -> Vec<Mock> {
        let mock_keys = self.mock_jwt_keys();
        let mock_token = self.mock_jwt_token(character_id, ownerhash);

        let mock_jwt_key_endpoint = self
            .setup
            .server
            .mock("GET", "/oauth/jwks")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_keys).unwrap())
            .create();

        let mock_jwt_token_endpoint = self
            .setup
            .server
            .mock("POST", "/v2/oauth/token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_token).unwrap())
            .create();

        vec![mock_jwt_key_endpoint, mock_jwt_token_endpoint]
    }
}
