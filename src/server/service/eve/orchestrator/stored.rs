use std::collections::HashMap;

use entity::{
    eve_alliance::Model as EveAllianceModel, eve_character::Model as EveCharacterModel,
    eve_corporation::Model as EveCorporationModel, eve_faction::Model as EveFactionModel,
};

use crate::server::error::Error;

/// Database models of entities stored by [`EveEntityOrchestrator`].
///
/// Provides access to the persisted database records with their generated IDs
/// and timestamps. Maps EVE Online IDs to both full database models and record IDs.
///
/// # Contents
///
/// - Full database models for entities that were fetched and stored
/// - Database record ID mappings for all entities (including those already in DB)
///
/// # Example
///
/// ```no_run
/// # use bifrost::server::service::eve::orchestrator::StoredEntities;
/// # fn example(stored: &StoredEntities, char_id: i64) {
/// if let Some(character) = stored.get_character(&char_id) {
///     println!("Character DB ID: {}, Created: {}", character.id, character.created_at);
/// }
///
/// // Get record ID for entities that may have already existed
/// if let Some(record_id) = stored.get_character_record_id(&char_id) {
///     println!("Character record ID: {}", record_id);
/// }
/// # }
/// ```
pub struct StoredEntities {
    // Maps EVE ID -> DB Model (only for entities that were fetched and stored)
    pub(super) factions_map: HashMap<i64, EveFactionModel>,
    pub(super) alliances_map: HashMap<i64, EveAllianceModel>,
    pub(super) corporations_map: HashMap<i64, EveCorporationModel>,
    pub(super) characters_map: HashMap<i64, EveCharacterModel>,

    // Maps EVE ID -> DB Record ID (includes pre-existing entities found during build)
    pub(super) factions_record_id_map: HashMap<i64, i32>,
    pub(super) alliances_record_id_map: HashMap<i64, i32>,
    pub(super) corporations_record_id_map: HashMap<i64, i32>,
    pub(super) characters_record_id_map: HashMap<i64, i32>,
}

// ===== Entity Getters =====
impl StoredEntities {
    /// Gets a character from the stored database models by character ID.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID
    ///
    /// # Returns
    /// - `Some(&EveCharacterModel)` - Character database model if it was stored
    /// - `None` - Character was not stored or was skipped
    pub fn get_character(&self, character_id: &i64) -> Option<&EveCharacterModel> {
        self.characters_map.get(character_id)
    }

    /// Gets a corporation from the stored database models by corporation ID.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID
    ///
    /// # Returns
    /// - `Some(&EveCorporationModel)` - Corporation database model if it was stored
    /// - `None` - Corporation was not stored
    pub fn get_corporation(&self, corporation_id: &i64) -> Option<&EveCorporationModel> {
        self.corporations_map.get(corporation_id)
    }

    /// Gets an alliance from the stored database models by alliance ID.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID
    ///
    /// # Returns
    /// - `Some(&EveAllianceModel)` - Alliance database model if it was stored
    /// - `None` - Alliance was not stored
    pub fn get_alliance(&self, alliance_id: &i64) -> Option<&EveAllianceModel> {
        self.alliances_map.get(alliance_id)
    }

    /// Gets all factions from the stored database models.
    ///
    /// Returns all faction models that were stored. Useful for bulk faction operations.
    ///
    /// # Returns
    /// - `Vec<EveFactionModel>` - All stored faction database models, or empty vec if:
    ///   - No factions were requested in the builder
    ///
    /// Note: This returns factions even if they were loaded due to being up-to-date
    /// or returned 304 Not Modified
    pub fn get_all_factions(&self) -> Vec<EveFactionModel> {
        self.factions_map.values().cloned().collect()
    }
}

// ===== Entity Getters (Fallible) =====
impl StoredEntities {
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
    pub fn get_character_or_err(&self, character_id: &i64) -> Result<&EveCharacterModel, Error> {
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
        corporation_id: &i64,
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
    pub fn get_alliance_or_err(&self, alliance_id: &i64) -> Result<&EveAllianceModel, Error> {
        self.get_alliance(alliance_id).ok_or_else(|| {
            Error::InternalError(format!(
                "Failed to retrieve information for alliance {} from database after fetching from ESI & storing.",
                alliance_id
            ))
        })
    }
}

// ===== Record ID Getters =====
impl StoredEntities {
    /// Gets a character's database record ID by character ID.
    ///
    /// Returns the database record ID for characters that were either fetched and stored,
    /// or found to already exist in the database during the build process.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID
    ///
    /// # Returns
    /// - `Some(i32)` - Database record ID if the character exists in the database
    /// - `None` - Character was not involved in this orchestrator operation
    pub fn get_character_record_id(&self, character_id: &i64) -> Option<i32> {
        self.characters_record_id_map.get(character_id).copied()
    }

    /// Gets a corporation's database record ID by corporation ID.
    ///
    /// Returns the database record ID for corporations that were either fetched and stored,
    /// or found to already exist in the database during the build process.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID
    ///
    /// # Returns
    /// - `Some(i32)` - Database record ID if the corporation exists in the database
    /// - `None` - Corporation was not involved in this orchestrator operation
    pub fn get_corporation_record_id(&self, corporation_id: &i64) -> Option<i32> {
        self.corporations_record_id_map.get(corporation_id).copied()
    }

    /// Gets an alliance's database record ID by alliance ID.
    ///
    /// Returns the database record ID for alliances that were either fetched and stored,
    /// or found to already exist in the database during the build process.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID
    ///
    /// # Returns
    /// - `Some(i32)` - Database record ID if the alliance exists in the database
    /// - `None` - Alliance was not involved in this orchestrator operation
    pub fn get_alliance_record_id(&self, alliance_id: &i64) -> Option<i32> {
        self.alliances_record_id_map.get(alliance_id).copied()
    }

    /// Gets a faction's database record ID by faction ID.
    ///
    /// Returns the database record ID for factions that were either fetched and stored,
    /// or found to already exist in the database during the build process.
    ///
    /// # Arguments
    /// - `faction_id` - EVE Online faction ID
    ///
    /// # Returns
    /// - `Some(i32)` - Database record ID if the faction exists in the database
    /// - `None` - Faction was not involved in this orchestrator operation
    pub fn get_faction_record_id(&self, faction_id: &i64) -> Option<i32> {
        self.factions_record_id_map.get(faction_id).copied()
    }
}
