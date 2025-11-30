use std::collections::{HashMap, HashSet};
use std::time::Instant;

use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};

use crate::server::error::Error;

/// Wrapper around DatabaseTransaction that tracks when it was created
///
/// This allows the cache to automatically detect when a new transaction is being used
/// (indicating a retry attempt) and clear the database model caches accordingly.
pub struct TrackedTransaction {
    txn: DatabaseTransaction,
    pub created_at: Instant,
}

impl TrackedTransaction {
    /// Create a new tracked transaction from a database connection
    pub async fn begin(db: &DatabaseConnection) -> Result<Self, sea_orm::DbErr> {
        Ok(Self {
            txn: db.begin().await?,
            created_at: Instant::now(),
        })
    }

    /// Commit the underlying transaction
    pub async fn commit(self) -> Result<(), sea_orm::DbErr> {
        self.txn.commit().await
    }

    /// Get a reference to the underlying transaction
    pub fn as_ref(&self) -> &DatabaseTransaction {
        &self.txn
    }
}

/// Unified orchestration cache - single source of truth for all EVE entity data
///
/// This cache is designed to be shared across all orchestrators to prevent duplicate
/// fetching of data from the database or ESI. Idempotent persistence is achieved by
/// checking which models exist in the database model cache before persisting.
///
/// # Usage
///
/// Create a single `OrchestrationCache` instance and pass it (by mutable reference) to
/// all orchestrators that need to work together. On transaction rollback, ensure the
/// database model caches are cleared to allow retry attempts to persist correctly.</parameter>
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

    // Transaction tracking - used to detect when a new transaction is being used
    transaction_id: Option<Instant>,
}

impl OrchestrationCache {
    /// Check if the provided transaction ID differs from the cached one, and if so,
    /// clear all database model caches automatically.
    ///
    /// This enables automatic cache clearing on transaction retries without requiring
    /// explicit calls to clear the cache. Simply call this method at the start of each
    /// persistence operation, passing the transaction's creation timestamp.
    ///
    /// # Arguments
    /// - `txn_id`: Timestamp when the current transaction was created
    ///
    /// # Returns
    /// - `true` if caches were cleared (new transaction detected)
    /// - `false` if caches were not cleared (same transaction)
    pub fn check_and_clear_on_new_transaction(&mut self, txn_id: Instant) -> bool {
        if let Some(cached_id) = self.transaction_id {
            if cached_id != txn_id {
                // New transaction detected, clear database model caches
                self.clear_db_model_caches();
                self.transaction_id = Some(txn_id);
                return true;
            }
        } else {
            // First transaction, just record it
            self.transaction_id = Some(txn_id);
        }
        false
    }

    /// Clear all database model caches manually
    ///
    /// This is now primarily used internally by `check_and_clear_on_new_transaction`.
    /// You generally don't need to call this directly - instead use transaction tracking.
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
    ///     // Clear database model caches before retry
    ///     cache.clear_db_model_caches();
    ///     let retry_result = perform_transaction(&mut cache).await;
    /// }
    /// ```
    pub fn clear_db_model_caches(&mut self) {
        self.faction_model.clear();
        self.alliance_model.clear();
        self.corporation_model.clear();
        self.character_model.clear();
        self.faction_db_id.clear();
        self.alliance_db_id.clear();
        self.corporation_db_id.clear();
        self.character_db_id.clear();
        self.transaction_id = None;
    }

    /// Persist all ESI models from cache in dependency order
    ///
    /// This method persists all fetched ESI data in the cache to the database,
    /// respecting foreign key dependencies (factions -> alliances -> corporations -> characters).
    /// Only ESI models not already present in the database model cache will be persisted.
    ///
    /// # Arguments
    /// - `db` - Database connection for creating orchestrators
    /// - `esi_client` - ESI client for creating orchestrators
    /// - `txn` - Tracked database transaction to persist within
    ///
    /// # Returns
    /// - `Ok(())` if all entities were successfully persisted
    /// - `Err(Error)` if any persistence operation failed
    pub async fn persist_all(
        &mut self,
        db: &DatabaseConnection,
        esi_client: &eve_esi::Client,
        txn: &TrackedTransaction,
    ) -> Result<(), Error> {
        use super::{
            alliance::AllianceOrchestrator, character::CharacterOrchestrator,
            corporation::CorporationOrchestrator, faction::FactionOrchestrator,
        };

        // Persist in dependency order: factions -> alliances -> corporations -> characters
        if !self.faction_esi.is_empty() {
            let faction_orch = FactionOrchestrator::new(db, esi_client);
            faction_orch.persist_cached_factions(txn, self).await?;
        }

        if !self.alliance_esi.is_empty() {
            let alliance_orch = AllianceOrchestrator::new(db, esi_client);
            alliance_orch.persist_cached_alliances(txn, self).await?;
        }

        if !self.corporation_esi.is_empty() {
            let corporation_orch = CorporationOrchestrator::new(db, esi_client);
            corporation_orch
                .persist_cached_corporations(txn, self)
                .await?;
        }

        if !self.character_esi.is_empty() {
            let character_orch = CharacterOrchestrator::new(db, esi_client);
            character_orch.persist_cached_characters(txn, self).await?;
        }

        Ok(())
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
