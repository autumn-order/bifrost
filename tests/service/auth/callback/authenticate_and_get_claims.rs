//! Tests for CallbackService::authenticate_and_get_claims method.
//!
//! This module verifies the authentication and JWT claims extraction logic,
//! including successful token exchange, JWT validation, and error handling
//! for invalid authorization codes.

use bifrost::server::{error::Error, service::auth::callback::CallbackService};
use bifrost_test_utils::prelude::*;

/// Tests successful authentication and JWT claims extraction.
///
/// Verifies that the service successfully exchanges an authorization code
/// for an access token and extracts valid JWT claims containing character
/// information and owner hash.
///
/// Expected: Ok with EveJwtClaims containing correct character_id and owner hash
#[tokio::test]
async fn returns_valid_jwt_claims() -> Result<(), TestError> {
    let character_id = 123456789;
    let owner_hash = "test_owner_hash_123";

    let mut test = TestBuilder::new().build().await?;

    // Create JWT endpoints that will return tokens for the specified character
    test.auth().create_jwt_endpoints(character_id, owner_hash);

    let authorization_code = "mock_auth_code";

    // Call the authenticate_and_get_claims method
    let result =
        CallbackService::authenticate_and_get_claims(&test.esi_client, authorization_code).await;

    assert!(result.is_ok());
    let claims = result.unwrap();

    // Verify the claims contain the correct character ID
    assert_eq!(claims.character_id().unwrap(), character_id);

    // Verify the claims contain the correct owner hash
    assert_eq!(claims.owner, owner_hash);

    test.assert_mocks();

    Ok(())
}

/// Tests authentication with multiple different characters.
///
/// Verifies that the service correctly handles authentication for different
/// characters with different owner hashes, ensuring no cross-contamination
/// between authentication sessions.
///
/// Expected: Ok with correct character_id and owner hash for each call
#[tokio::test]
async fn handles_multiple_characters() -> Result<(), TestError> {
    let character_id_1 = 111111111;
    let owner_hash_1 = "owner_hash_alpha";

    let character_id_2 = 222222222;
    let owner_hash_2 = "owner_hash_beta";

    let mut test = TestBuilder::new().build().await?;

    // Create JWT endpoints for first character
    test.auth()
        .create_jwt_endpoints(character_id_1, owner_hash_1);

    let result_1 =
        CallbackService::authenticate_and_get_claims(&test.esi_client, "auth_code_1").await;

    assert!(result_1.is_ok());
    let claims_1 = result_1.unwrap();
    assert_eq!(claims_1.character_id().unwrap(), character_id_1);
    assert_eq!(claims_1.owner, owner_hash_1);

    // Create JWT endpoints for second character
    test.auth()
        .create_jwt_endpoints(character_id_2, owner_hash_2);

    let result_2 =
        CallbackService::authenticate_and_get_claims(&test.esi_client, "auth_code_2").await;

    assert!(result_2.is_ok());
    let claims_2 = result_2.unwrap();
    assert_eq!(claims_2.character_id().unwrap(), character_id_2);
    assert_eq!(claims_2.owner, owner_hash_2);

    test.assert_mocks();

    Ok(())
}

/// Tests error handling when OAuth2 token endpoint is unavailable.
///
/// Verifies that the service returns an appropriate error when the ESI OAuth2
/// token endpoint fails to respond or is unavailable.
///
/// Expected: Err with EsiError
#[tokio::test]
async fn fails_when_token_endpoint_unavailable() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    // No JWT endpoints configured - token exchange will fail

    let authorization_code = "mock_auth_code";

    let result =
        CallbackService::authenticate_and_get_claims(&test.esi_client, authorization_code).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::EsiError(_)));

    Ok(())
}

/// Tests authentication with various owner hash formats.
///
/// Verifies that the service correctly handles different owner hash string
/// formats, including hashes with special characters and different lengths.
///
/// Expected: Ok with correct owner hash preserved exactly as provided
#[tokio::test]
async fn preserves_owner_hash_format() -> Result<(), TestError> {
    let test_cases = vec![
        ("simple_hash", 100000001),
        ("hash-with-dashes", 100000002),
        ("hash_with_underscores", 100000003),
        ("UPPERCASE_HASH", 100000004),
        ("MixedCase123", 100000005),
        ("hash.with.dots", 100000006),
        (
            "very_long_hash_string_with_many_characters_0123456789",
            100000007,
        ),
    ];

    for (owner_hash, character_id) in test_cases {
        let mut test = TestBuilder::new().build().await?;

        test.auth().create_jwt_endpoints(character_id, owner_hash);

        let result =
            CallbackService::authenticate_and_get_claims(&test.esi_client, "auth_code").await;

        assert!(result.is_ok());
        let claims = result.unwrap();
        assert_eq!(claims.owner, owner_hash);
        assert_eq!(claims.character_id().unwrap(), character_id);

        test.assert_mocks();
    }

    Ok(())
}
