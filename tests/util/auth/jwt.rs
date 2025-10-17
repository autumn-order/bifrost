use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use eve_esi::model::oauth2::{EveJwtClaims, EveJwtKey, EveJwtKeys};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use oauth2::basic::BasicTokenType;
use oauth2::{AccessToken, EmptyExtraTokenFields, RefreshToken, StandardTokenResponse};
use openssl::rsa::Rsa;

pub static RSA_KEY_ID: &str = "JWT-Signature-Key-1";

/// Creates mock JWT keys used for validating a JWT token during an OAuth2 callback
pub fn create_mock_jwt_keys() -> EveJwtKeys {
    let public_key = include_bytes!("./public_test_rsa_key.pem");
    let rsa = Rsa::public_key_from_pem(public_key).unwrap();

    // Get the modulus and exponent as raw bytes which are used for the validation
    let n_bytes = rsa.n().to_vec();
    let e_bytes = rsa.e().to_vec();

    // Base64URL encode the modulus & exponent
    let n = URL_SAFE_NO_PAD.encode(n_bytes);
    let e = URL_SAFE_NO_PAD.encode(e_bytes);

    EveJwtKeys {
        skip_unresolved_json_web_keys: false,
        keys: vec![
            EveJwtKey::RS256 {
                e: e,
                kid: RSA_KEY_ID.to_string(),
                kty: "RSA".to_string(),
                n: n,
                r#use: "sig".to_string(),
            },
            // Not actually used but EVE's API does return an ES256 key alongside the RS256 so it is included
            EveJwtKey::ES256 {
                crv: "P-256".to_string(),
                kid: "JWT-Signature-Key-2".to_string(),
                kty: "EC".to_string(),
                r#use: "sig".to_string(),
                x: "ITcDYJ8WVpDO4QtZ169xXUt7GB1Y6-oMKIwJ3nK1tFU".to_string(),
                y: "ZAJr0f4V2Eu7xBgLMgQBdJ2DZ2mp8JykOhX4XgU_UEY".to_string(),
            },
        ],
    }
}

/// Creates mock JWT token for callback endpoint after successful login
pub fn create_mock_jwt_token(
    claims: EveJwtClaims,
) -> StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType> {
    let private_key = include_bytes!("./private_test_rsa_key.pem");
    let encoding_key =
        EncodingKey::from_rsa_pem(private_key).expect("Failed to create encoding key");

    let mut header = Header::new(Algorithm::RS256);

    header.kid = Some(RSA_KEY_ID.to_string());

    let access_token_secret =
        encode(&header, &claims, &encoding_key).expect("Failed to encode token");

    // Create the token components
    let access_token = AccessToken::new(access_token_secret);
    let token_type = BasicTokenType::Bearer;
    let expires_in = Some(&Duration::from_secs(3600)); // 1 hour

    // We aren't actually validating this refresh token in get_token_refresh tests,
    // we just use this for the get_token_refresh argument to test the execution paths.
    let refresh_token = Some(RefreshToken::new("mock_refresh_token_value".to_string()));

    // Create empty extra fields
    let extra_fields = EmptyExtraTokenFields {};

    // Create the token response
    let mut token = StandardTokenResponse::new(access_token, token_type, extra_fields);

    // Set optional fields
    token.set_expires_in(expires_in);
    token.set_refresh_token(refresh_token);

    token
}
