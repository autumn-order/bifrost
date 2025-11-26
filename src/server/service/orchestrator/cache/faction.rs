use std::collections::HashMap;

use eve_esi::model::universe::Faction;

/// Cache for faction orchestration
/// Contains caches for faction data
#[derive(Clone, Default, Debug)]
pub struct FactionOrchestrationCache {
    pub faction_esi: HashMap<i64, Faction>,
    pub faction_model: HashMap<i64, entity::eve_faction::Model>,
    pub faction_db_id: HashMap<i64, i32>,
    // Faction orchestrator is often utilized in multiple places due to it being
    // depended upon by alliances, corporations, and characters. This prevents
    // redundantly attempting to persist factions multiple times after a successful
    // fetch.
    pub already_persisted: bool,
}

impl FactionOrchestrationCache {
    /// Reset the `already_persisted` flag for retrying a transaction
    pub fn reset_persistence_flag(&mut self) {
        self.already_persisted = false
    }
}
