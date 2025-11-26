use std::collections::{HashMap, HashSet};

use eve_esi::model::alliance::Alliance;

use crate::server::service::orchestrator::cache::faction::FactionOrchestrationCache;

#[derive(Clone, Default, Debug)]
pub struct AllianceOrchestrationCache {
    pub alliance_esi: HashMap<i64, Alliance>,
    pub alliance_model: HashMap<i64, entity::eve_alliance::Model>,
    pub alliance_db_id: HashMap<i64, i32>,
    pub faction: FactionOrchestrationCache,
    /// Flag preventing duplicate persistence of alliances should
    /// the persist method be called within multiple other orchestrators
    pub already_persisted: bool,
}

impl AllianceOrchestrationCache {
    /// Reset the `already_persisted` flag for retrying a transaction
    pub fn reset_persistence_flag(&mut self) {
        self.already_persisted = false
    }

    /// Extract list of dependent faction IDs for list of alliances
    pub fn get_faction_dependency_ids(&self, alliances: &[&Alliance]) -> Vec<i64> {
        alliances
            .iter()
            .filter_map(|a| a.faction_id)
            .collect::<HashSet<i64>>()
            .into_iter()
            .collect::<Vec<i64>>()
    }
}
