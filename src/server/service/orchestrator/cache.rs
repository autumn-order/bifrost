use std::collections::{HashMap, HashSet};

use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};

/// Unified orchestration cache - single source of truth for all EVE entity data
///
/// This cache is designed to be shared across all orchestrators to prevent duplicate
/// fetching of data from the database or ESI. It includes an embedded persistence tracker
/// to enable idempotent persistence operations.
///
/// # Usage
///
/// Create a single `OrchestrationCache` instance and pass it (by mutable reference) to
/// all orchestrators that need to work together. Before retrying a transaction, call
/// `reset_persistence_flags()` to ensure all entities are persisted again in the new attempt.
#[derive(Clone, Default, Debug)]
pub struct OrchestrationCache {
    // Faction data
    pub faction_esi: HashMap<i64, Faction>,
    pub faction_model: HashMap<i64, entity::eve_faction::Model>,
    pub faction_db_id: HashMap<i64, i32>,

    // Alliance data
    pub alliance_esi: HashMap<i64, Alliance>,
    pub alliance_model: HashMap<i64, entity::eve_alliance::Model>,
    pub alliance_db_id: HashMap<i64, i32>,

    // Corporation data
    pub corporation_esi: HashMap<i64, Corporation>,
    pub corporation_model: HashMap<i64, entity::eve_corporation::Model>,
    pub corporation_db_id: HashMap<i64, i32>,

    // Character data
    pub character_esi: HashMap<i64, Character>,
    pub character_model: HashMap<i64, entity::eve_character::Model>,
    pub character_db_id: HashMap<i64, i32>,

    // Persistence tracking - tracks which entity types have been persisted in current transaction
    pub(super) factions_persisted: bool,
    pub(super) alliances_persisted: bool,
    pub(super) corporations_persisted: bool,
    pub(super) characters_persisted: bool,
}

impl OrchestrationCache {
    /// Reset all persistence flags for retry attempts
    ///
    /// This should be called before retrying a transaction to ensure that
    /// all entities are persisted again in the new transaction attempt.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut cache = OrchestrationCache::default();
    ///
    /// // Attempt transaction
    /// let result = perform_transaction(&mut cache).await;
    ///
    /// if result.is_err() {
    ///     // Reset before retry
    ///     cache.reset_persistence_flags();
    ///     let retry_result = perform_transaction(&mut cache).await;
    /// }
    /// ```
    pub fn reset_persistence_flags(&mut self) {
        self.factions_persisted = false;
        self.alliances_persisted = false;
        self.corporations_persisted = false;
        self.characters_persisted = false;
    }
}

/// Extract list of dependent faction IDs for list of alliances
pub fn get_alliance_faction_dependency_ids(alliances: &[&Alliance]) -> Vec<i64> {
    alliances
        .iter()
        .filter_map(|a| a.faction_id)
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect::<Vec<i64>>()
}

/// Extract list of dependent faction IDs for list of corporations
pub fn get_corporation_faction_dependency_ids(corporations: &[&Corporation]) -> Vec<i64> {
    corporations
        .iter()
        .filter_map(|c| c.faction_id)
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect::<Vec<i64>>()
}

/// Extract list of dependent alliance IDs for list of corporations
pub fn get_corporation_alliance_dependency_ids(corporations: &[&Corporation]) -> Vec<i64> {
    corporations
        .iter()
        .filter_map(|c| c.alliance_id)
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect::<Vec<i64>>()
}

/// Extract list of dependent faction IDs for list of characters
pub fn get_character_faction_dependency_ids(characters: &[&Character]) -> Vec<i64> {
    characters
        .iter()
        .filter_map(|c| c.faction_id)
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect::<Vec<i64>>()
}

/// Extract list of dependent corporation IDs for list of characters
pub fn get_character_corporation_dependency_ids(characters: &[&Character]) -> Vec<i64> {
    characters
        .iter()
        .map(|c| c.corporation_id)
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect::<Vec<i64>>()
}
