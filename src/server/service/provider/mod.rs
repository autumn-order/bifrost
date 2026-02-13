//! # EVE Online Entity Orchestration Provider

pub mod builder;
mod util;

use std::collections::HashMap;

use entity::{
    eve_alliance::Model as EveAllianceModel, eve_character::Model as EveCharacterModel,
    eve_corporation::Model as EveCorporationModel, eve_faction::Model as EveFactionModel,
};
use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};
use sea_orm::DatabaseTransaction;

use crate::server::error::Error;

pub struct EveEntityProvider {
    // ESI data (what was fetched)
    factions_map: Option<HashMap<i64, Faction>>,
    alliances_map: HashMap<i64, Alliance>,
    corporations_map: HashMap<i64, Corporation>,
    characters_map: HashMap<i64, Character>,

    // DB IDs for relationships (from DB check)
    faction_record_id_map: HashMap<i64, i32>,
    alliance_record_id_map: HashMap<i64, i32>,
    corporation_record_id_map: HashMap<i64, i32>,
}

impl EveEntityProvider {
    /// Stores all fetched entities without returning the stored data
    ///
    /// Useful for high volume bulk operations such as updating affiliations
    pub async fn store(&self, txn: &DatabaseTransaction) -> Result<(), Error> {
        // Store in dependency order: factions -> alliances -> corps -> chars
        // Only store entities that were fetched (not in *_db_ids)

        todo!()
    }

    /// Stores all fetched entities, returning the stored data
    pub async fn store_with_returning(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<StoredEntities, Error> {
        // Store in dependency order: factions -> alliances -> corps -> chars
        // Only store entities that were fetched (not in *_db_ids)

        todo!()
    }
}

/// Provides information related to entities stored by [`EveEntityProvider`]
pub struct StoredEntities {
    // Maps EVE ID -> DB Model
    pub factions: HashMap<i64, EveFactionModel>,
    pub alliances: HashMap<i64, EveAllianceModel>,
    pub corporations: HashMap<i64, EveCorporationModel>,
    pub characters: HashMap<i64, EveCharacterModel>,
}
impl StoredEntities {
    pub fn get_character(&self, character_id: i64) -> Option<&EveCharacterModel> {
        self.characters.get(&character_id)
    }

    pub fn get_corporation(&self, corporation_id: i64) -> Option<&EveCorporationModel> {
        self.corporations.get(&corporation_id)
    }

    pub fn get_alliance(&self, alliance_id: i64) -> Option<&EveAllianceModel> {
        self.alliances.get(&alliance_id)
    }

    pub fn get_faction(&self, faction_id: i64) -> Option<&EveFactionModel> {
        self.factions.get(&faction_id)
    }

    // Similar accessors for other types...
}
