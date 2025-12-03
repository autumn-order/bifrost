//! Factory functions for generating mock authentication objects.
//!
//! Provides pure functions for creating EVE Online SSO authentication objects
//! (JWT claims) with standard test values. These are used for testing OAuth2
//! authentication flows.

use chrono::Utc;
use eve_esi::model::oauth2::EveJwtClaims;

/// Create mock JWT claims with default test values.
///
/// Returns an EveJwtClaims struct populated with standard test data matching
/// the structure returned by EVE Online's SSO service. The claims include a
/// 15-minute expiration time and current timestamp for issued-at time.
///
/// # Arguments
/// - `character_id` - The EVE Online character ID to include in the claims
/// - `owner_hash` - The owner hash for ownership verification
///
/// # Returns
/// - `EveJwtClaims` - JWT claims object with test data
pub fn mock_jwt_claims(character_id: i64, owner_hash: &str) -> EveJwtClaims {
    let now = Utc::now();
    EveJwtClaims {
        iss: "https://login.eveonline.com".to_string(),
        sub: format!("CHARACTER:EVE:{}", character_id),
        aud: vec!["client_id".to_string()],
        jti: "test_jti".to_string(),
        kid: "test_kid".to_string(),
        tenant: "tranquility".to_string(),
        region: "world".to_string(),
        exp: now + chrono::Duration::seconds(900),
        iat: now,
        scp: vec![],
        name: "Test Character".to_string(),
        owner: owner_hash.to_string(),
        azp: "test_azp".to_string(),
    }
}
