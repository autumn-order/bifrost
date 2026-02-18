use std::collections::HashMap;

use entity::{
    eve_alliance::Model as EveAllianceModel, eve_character::Model as EveCharacterModel,
    eve_corporation::Model as EveCorporationModel, eve_faction::Model as EveFactionModel,
};

use crate::server::error::Error;

/// Database models of entities stored by [`EveEntityProvider`].
///
/// Provides access to the persisted database records with their generated IDs
/// and timestamps. Maps EVE Online IDs to database models.
///
/// # Example
///
/// ```no_run
/// # use bifrost::server::service::provider::StoredEntities;
/// # fn example(stored: &StoredEntities, char_id: i64) {
/// if let Some(character) = stored.get_character(char_id) {
///     println!("Character DB ID: {}, Created: {}", character.id, character.created_at);
/// }
/// # }
/// ```
pub struct StoredEntities {
    // Maps EVE ID -> DB Model
    pub(super) factions_map: HashMap<i64, EveFactionModel>,
    pub(super) alliances_map: HashMap<i64, EveAllianceModel>,
    pub(super) corporations_map: HashMap<i64, EveCorporationModel>,
    pub(super) characters_map: HashMap<i64, EveCharacterModel>,
}

impl StoredEntities {
    /// Gets a character from the stored database models by character ID.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID
    ///
    /// # Returns
    /// - `Some(&EveCharacterModel)` - Character database model if it was stored
    /// - `None` - Character was not stored or was skipped
    pub fn get_character(&self, character_id: i64) -> Option<&EveCharacterModel> {
        self.characters_map.get(&character_id)
    }

    /// Gets a corporation from the stored database models by corporation ID.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID
    ///
    /// # Returns
    /// - `Some(&EveCorporationModel)` - Corporation database model if it was stored
    /// - `None` - Corporation was not stored
    pub fn get_corporation(&self, corporation_id: i64) -> Option<&EveCorporationModel> {
        self.corporations_map.get(&corporation_id)
    }

    /// Gets an alliance from the stored database models by alliance ID.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID
    ///
    /// # Returns
    /// - `Some(&EveAllianceModel)` - Alliance database model if it was stored
    /// - `None` - Alliance was not stored
    pub fn get_alliance(&self, alliance_id: i64) -> Option<&EveAllianceModel> {
        self.alliances_map.get(&alliance_id)
    }

    /// Gets a character from the stored database models by character ID, or returns an error.
    ///
    /// This is a convenience method that wraps [`get_character`](Self::get_character) and
    /// converts `None` into a descriptive `InternalError`.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID
    ///
    /// # Returns
    /// - `Ok(&EveCharacterModel)` - Character database model if it was stored
    /// - `Err(Error::InternalError)` - Character was not found after storing
    pub fn get_character_or_err(&self, character_id: i64) -> Result<&EveCharacterModel, Error> {
        self.get_character(character_id).ok_or_else(|| {
            Error::InternalError(format!(
                "Failed to retrieve information for character {} from database after fetching from ESI & storing.",
                character_id
            ))
        })
    }

    /// Gets a corporation from the stored database models by corporation ID, or returns an error.
    ///
    /// This is a convenience method that wraps [`get_corporation`](Self::get_corporation) and
    /// converts `None` into a descriptive `InternalError`.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID
    ///
    /// # Returns
    /// - `Ok(&EveCorporationModel)` - Corporation database model if it was stored
    /// - `Err(Error::InternalError)` - Corporation was not found after storing
    pub fn get_corporation_or_err(
        &self,
        corporation_id: i64,
    ) -> Result<&EveCorporationModel, Error> {
        self.get_corporation(corporation_id).ok_or_else(|| {
            Error::InternalError(format!(
                "Failed to retrieve information for corporation {} from database after fetching from ESI & storing.",
                corporation_id
            ))
        })
    }

    /// Gets an alliance from the stored database models by alliance ID, or returns an error.
    ///
    /// This is a convenience method that wraps [`get_alliance`](Self::get_alliance) and
    /// converts `None` into a descriptive `InternalError`.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID
    ///
    /// # Returns
    /// - `Ok(&EveAllianceModel)` - Alliance database model if it was stored
    /// - `Err(Error::InternalError)` - Alliance was not found after storing
    pub fn get_alliance_or_err(&self, alliance_id: i64) -> Result<&EveAllianceModel, Error> {
        self.get_alliance(alliance_id).ok_or_else(|| {
            Error::InternalError(format!(
                "Failed to retrieve information for alliance {} from database after fetching from ESI & storing.",
                alliance_id
            ))
        })
    }

    /// Gets all factions from the stored database models.
    ///
    /// Returns all faction models that were stored. Useful for bulk faction operations.
    ///
    /// # Returns
    /// - `Vec<EveFactionModel>` - All stored faction database models, or empty vec if:
    ///   - Factions returned 304 Not Modified (only timestamps were updated)
    ///   - Factions were not stale (no fetch was needed)
    ///   - No factions were requested in the builder
    pub fn get_all_factions(&self) -> Vec<EveFactionModel> {
        self.factions_map.values().cloned().collect()
    }
}
