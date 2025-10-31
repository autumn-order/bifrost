use mockito::Mock;

use crate::fixtures::auth::AuthFixtures;

impl<'a> AuthFixtures<'a> {
    pub fn with_jwt_endpoints(&mut self, character_id: i64, ownerhash: &str) -> Vec<Mock> {
        let mock_keys = self.with_mock_jwt_keys();
        let mock_token = self.with_mock_jwt_token(character_id, ownerhash);

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
