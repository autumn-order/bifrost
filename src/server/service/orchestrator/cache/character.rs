use std::collections::{HashMap, HashSet};

use eve_esi::model::character::Character;

use crate::server::service::orchestrator::cache::{
    alliance::AllianceOrchestrationCache, corporation::CorporationOrchestrationCache,
    faction::FactionOrchestrationCache,
};

#[derive(Clone, Default, Debug)]
pub struct CharacterOrchestrationCache {
    pub character_esi: HashMap<i64, Character>,
    pub character_model: HashMap<i64, entity::eve_character::Model>,
    pub character_db_id: HashMap<i64, i32>,
    pub faction: FactionOrchestrationCache,
    pub alliance: AllianceOrchestrationCache,
    pub corporation: CorporationOrchestrationCache,
    /// Flag preventing duplicate persistence of characters should
    /// the persist method be called within multiple other orchestrators
    pub already_persisted: bool,
}

impl CharacterOrchestrationCache {
    /// Reset the `already_persisted` flag for retrying a transaction
    pub fn reset_persistence_flag(&mut self) {
        self.already_persisted = false
    }

    /// Extract list of dependent faction IDs for list of characters
    pub fn get_faction_dependency_ids(&self, characters: &[&Character]) -> Vec<i64> {
        characters
            .iter()
            .filter_map(|c| c.faction_id)
            .collect::<HashSet<i64>>()
            .into_iter()
            .collect::<Vec<i64>>()
    }

    /// Extract list of dependent corporation IDs for list of characters
    pub fn get_corporation_dependency_ids(&self, characters: &[&Character]) -> Vec<i64> {
        characters
            .iter()
            .map(|c| c.corporation_id)
            .collect::<HashSet<i64>>()
            .into_iter()
            .collect::<Vec<i64>>()
    }
}
