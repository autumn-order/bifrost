use std::collections::{HashMap, HashSet};

use eve_esi::model::corporation::Corporation;

use crate::server::service::orchestrator::cache::{
    alliance::AllianceOrchestrationCache, faction::FactionOrchestrationCache,
};

#[derive(Clone, Default, Debug)]
pub struct CorporationOrchestrationCache {
    pub corporation_esi: HashMap<i64, Corporation>,
    pub corporation_model: HashMap<i64, entity::eve_corporation::Model>,
    pub corporation_db_id: HashMap<i64, i32>,
    pub faction: FactionOrchestrationCache,
    pub alliance: AllianceOrchestrationCache,
    /// Flag preventing duplicate persistence of corporations should
    /// the persist method be called within multiple other orchestrators
    pub already_persisted: bool,
}

impl CorporationOrchestrationCache {
    /// Reset the `already_persisted` flag for retrying a transaction
    pub fn reset_persistence_flag(&mut self) {
        self.already_persisted = false
    }

    /// Extract list of dependent faction IDs for list of corporations
    pub fn get_faction_dependency_ids(&self, corporations: &[&Corporation]) -> Vec<i64> {
        corporations
            .iter()
            .filter_map(|a| a.faction_id)
            .collect::<HashSet<i64>>()
            .into_iter()
            .collect::<Vec<i64>>()
    }

    /// Extract list of dependent alliance IDs for list of corporations
    pub fn get_alliance_dependency_ids(&self, corporations: &[&Corporation]) -> Vec<i64> {
        corporations
            .iter()
            .filter_map(|a| a.alliance_id)
            .collect::<HashSet<i64>>()
            .into_iter()
            .collect::<Vec<i64>>()
    }
}
