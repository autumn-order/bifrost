//! EVE entity mock data generation utilities.
//!
//! This module provides methods for generating mock EVE Online entity objects
//! with standard test values. These methods create in-memory data objects without
//! database side effects, useful for testing ESI endpoint responses.

use eve_esi::model::{
    alliance::Alliance,
    character::{Character, CharacterAffiliation},
    corporation::Corporation,
    universe::Faction,
};

use crate::fixtures::eve::{factory, EveFixtures};

impl<'a> EveFixtures<'a> {
    /// Generate a mock faction object with test values.
    ///
    /// Creates a Faction struct populated with standard test data. This is a pure
    /// data generation method with no database or HTTP side effects.
    ///
    /// # Arguments
    /// - `faction_id` - The EVE Online faction ID to use
    ///
    /// # Returns
    /// - `Faction` - A faction object with test data
    pub fn mock_faction(&self, faction_id: i64) -> Faction {
        factory::mock_faction(faction_id)
    }

    /// Generate a mock alliance object with test values.
    ///
    /// Creates an Alliance struct populated with standard test data. Returns both
    /// the alliance ID and the alliance object for convenience. This is a pure
    /// data generation method with no database or HTTP side effects.
    ///
    /// # Arguments
    /// - `alliance_id` - The EVE Online alliance ID to use
    /// - `faction_id` - Optional faction ID the alliance belongs to
    ///
    /// # Returns
    /// - `(i64, Alliance)` - Tuple of alliance ID and alliance object with test data
    pub fn mock_alliance(&self, alliance_id: i64, faction_id: Option<i64>) -> (i64, Alliance) {
        (alliance_id, factory::mock_alliance(faction_id))
    }

    /// Generate a mock corporation object with test values.
    ///
    /// Creates a Corporation struct populated with standard test data. Returns both
    /// the corporation ID and the corporation object for convenience. This is a pure
    /// data generation method with no database or HTTP side effects.
    ///
    /// # Arguments
    /// - `corporation_id` - The EVE Online corporation ID to use
    /// - `alliance_id` - Optional alliance ID the corporation belongs to
    /// - `faction_id` - Optional faction ID the corporation belongs to
    ///
    /// # Returns
    /// - `(i64, Corporation)` - Tuple of corporation ID and corporation object with test data
    pub fn mock_corporation(
        &self,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> (i64, Corporation) {
        (
            corporation_id,
            factory::mock_corporation(alliance_id, faction_id),
        )
    }

    /// Generate a mock character object with test values.
    ///
    /// Creates a Character struct populated with standard test data. Returns both
    /// the character ID and the character object for convenience. This is a pure
    /// data generation method with no database or HTTP side effects.
    ///
    /// # Arguments
    /// - `character_id` - The EVE Online character ID to use
    /// - `corporation_id` - The corporation ID the character belongs to
    /// - `alliance_id` - Optional alliance ID the character belongs to
    /// - `faction_id` - Optional faction ID the character belongs to
    ///
    /// # Returns
    /// - `(i64, Character)` - Tuple of character ID and character object with test data
    pub fn mock_character(
        &self,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> (i64, Character) {
        (
            character_id,
            factory::mock_character(corporation_id, alliance_id, faction_id),
        )
    }

    /// Generate a mock character affiliation object.
    ///
    /// Creates a CharacterAffiliation struct with the specified IDs. Used for
    /// testing character affiliation endpoint responses. This is a pure data
    /// generation method with no database or HTTP side effects.
    ///
    /// # Arguments
    /// - `character_id` - The EVE Online character ID
    /// - `corporation_id` - The corporation ID the character belongs to
    /// - `alliance_id` - Optional alliance ID the character belongs to
    /// - `faction_id` - Optional faction ID the character belongs to
    ///
    /// # Returns
    /// - `CharacterAffiliation` - A character affiliation object
    pub fn mock_character_affiliation(
        &self,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> CharacterAffiliation {
        factory::mock_character_affiliation(character_id, corporation_id, alliance_id, faction_id)
    }
}
