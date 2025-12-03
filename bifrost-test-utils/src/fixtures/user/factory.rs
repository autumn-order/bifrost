//! Factory functions for generating mock user database models.
//!
//! Provides pure functions for creating user and character ownership database models
//! with standard test values. These are in-memory model instances that don't require
//! database interaction, suitable for unit tests.

use chrono::Utc;

use crate::model::{CharacterOwnershipModel, EveCharacterModel};

/// Create a mock character database model for testing.
///
/// Returns an EveCharacterModel with standard test values. This creates an in-memory
/// model instance without database interaction, suitable for unit tests.
///
/// # Arguments
/// - `character_id` - The EVE Online character ID
///
/// # Returns
/// - `EveCharacterModel` - A character model with test data
pub fn mock_character_model(character_id: i64) -> EveCharacterModel {
    let now = Utc::now().naive_utc();
    EveCharacterModel {
        id: 1,
        character_id,
        corporation_id: 1,
        faction_id: None,
        name: "Test Character".to_string(),
        birthday: now,
        gender: "male".to_string(),
        security_status: Some(0.0),
        title: None,
        bloodline_id: 1,
        race_id: 1,
        description: None,
        created_at: now,
        info_updated_at: now,
        affiliation_updated_at: now,
    }
}

/// Create a mock ownership database model for testing.
///
/// Returns a CharacterOwnershipModel with standard test values. This creates an
/// in-memory model instance without database interaction, suitable for unit tests.
///
/// # Arguments
/// - `user_id` - The user ID that owns the character
/// - `character_id` - The character record ID (not EVE character ID)
/// - `owner_hash` - The EVE Online owner hash for ownership verification
///
/// # Returns
/// - `CharacterOwnershipModel` - An ownership model with test data
pub fn mock_ownership_model(
    user_id: i32,
    character_id: i32,
    owner_hash: &str,
) -> CharacterOwnershipModel {
    let now = Utc::now().naive_utc();
    CharacterOwnershipModel {
        id: 1,
        user_id,
        character_id,
        owner_hash: owner_hash.to_string(),
        created_at: now,
        updated_at: now,
    }
}
