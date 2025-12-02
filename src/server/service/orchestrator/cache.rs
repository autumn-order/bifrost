//! Orchestration cache and transaction tracking for EVE data operations.
//!
//! This module provides the unified orchestration cache that serves as the single source of truth
//! for all EVE entity data during complex operations. It includes transaction tracking to automatically
//! handle cache invalidation during retry attempts, and utility functions for extracting dependency IDs.
//!
//! The cache is designed to prevent duplicate fetching from ESI or database, enable idempotent
//! persistence across retries, and maintain consistency across multiple orchestrator operations.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};

use crate::server::{
    error::Error,
    model::db::{EveAllianceModel, EveCharacterModel, EveCorporationModel, EveFactionModel},
};

/// Wrapper around DatabaseTransaction that tracks when it was created.
///
/// This allows the cache to automatically detect when a new transaction is being used
/// (indicating a retry attempt) and clear the database model caches accordingly.
///
/// The `created_at` timestamp serves as a unique identifier for each transaction attempt.
/// When the orchestration cache detects a different timestamp, it knows a new transaction
/// has started and automatically clears the database model caches to ensure retry attempts
/// can persist data correctly.
///
/// # Example
///
/// ```ignore
/// let mut cache = OrchestrationCache::default();
///
/// // First attempt
/// let txn = TrackedTransaction::begin(&db).await?;
/// let result = some_operation(&txn, &mut cache).await;
///
/// if result.is_err() {
///     // Transaction rolled back, try again with a new transaction
///     // Cache will automatically clear model caches when it sees the new timestamp
///     let txn = TrackedTransaction::begin(&db).await?;
///     let retry_result = some_operation(&txn, &mut cache).await?;
///     txn.commit().await?;
/// } else {
///     txn.commit().await?;
/// }
/// ```
pub struct TrackedTransaction {
    /// Underlying database transaction.
    txn: DatabaseTransaction,
    /// Timestamp when this transaction was created (for cache invalidation).
    pub created_at: Instant,
}

impl TrackedTransaction {
    /// Creates a new tracked transaction from a database connection.
    ///
    /// The transaction is timestamped with the current instant to enable
    /// automatic cache invalidation detection.
    ///
    /// # Arguments
    /// - `db` - Database connection to begin the transaction on
    ///
    /// # Returns
    /// - `Ok(TrackedTransaction)` - New tracked transaction
    /// - `Err(DbErr)` - Failed to begin database transaction
    pub async fn begin(db: &DatabaseConnection) -> Result<Self, sea_orm::DbErr> {
        Ok(Self {
            txn: db.begin().await?,
            created_at: Instant::now(),
        })
    }

    /// Commits the underlying database transaction.
    ///
    /// Consumes the tracked transaction to ensure it cannot be used after commit.
    ///
    /// # Returns
    /// - `Ok(())` - Transaction was committed successfully
    /// - `Err(DbErr)` - Failed to commit transaction
    pub async fn commit(self) -> Result<(), sea_orm::DbErr> {
        self.txn.commit().await
    }
}

impl AsRef<DatabaseTransaction> for TrackedTransaction {
    fn as_ref(&self) -> &DatabaseTransaction {
        &self.txn
    }
}

/// Unified orchestration cache - single source of truth for all EVE entity data.
///
/// This cache is designed to be shared across all orchestrators to prevent duplicate
/// fetching of data from the database or ESI. Idempotent persistence is achieved by
/// checking which models exist in the database model cache before persisting.
///
/// # Cache Structure
///
/// The cache maintains three types of data for each entity type:
/// - **ESI cache**: Raw data fetched from ESI API (e.g., `faction_esi`)
/// - **Model cache**: Database models after persistence (e.g., `faction_model`)
/// - **DB ID cache**: Mapping of EVE IDs to database entry IDs (e.g., `faction_db_id`)
///
/// # Transaction Tracking
///
/// The cache automatically tracks transaction timestamps and clears database model caches
/// when a new transaction is detected for the purposes of retries.
///
/// # Usage
///
/// Create a single `OrchestrationCache` instance and pass it (by mutable reference) to
/// all orchestrators that need to work together. The cache will automatically handle
/// clearing on transaction retries.
///
/// # Example
///
/// ```ignore
/// let mut cache = OrchestrationCache::default();
///
/// // Fetch entities from ESI
/// let character_orch = CharacterOrchestrator::new(&db, &esi_client);
/// character_orch.fetch_character(character_id, &mut cache).await?;
///
/// // Persist everything within a transaction
/// let txn = TrackedTransaction::begin(&db).await?;
/// cache.persist_all(&db, &esi_client, &txn).await?;
/// txn.commit().await?;
/// ```
#[derive(Clone, Default, Debug)]
pub struct OrchestrationCache {
    // Faction data
    /// ESI faction data cache (faction_id -> ESI Faction).
    pub faction_esi: HashMap<i64, Faction>,
    /// Persisted faction database models (faction_id -> Model).
    pub faction_model: HashMap<i64, EveFactionModel>,
    /// Faction database entry IDs (faction_id -> database primary key).
    pub faction_db_id: HashMap<i64, i32>,

    // Alliance data
    /// ESI alliance data cache (alliance_id -> ESI Alliance).
    pub alliance_esi: HashMap<i64, Alliance>,
    /// Persisted alliance database models (alliance_id -> Model).
    pub alliance_model: HashMap<i64, EveAllianceModel>,
    /// Alliance database entry IDs (alliance_id -> database primary key).
    pub alliance_db_id: HashMap<i64, i32>,

    // Corporation data
    /// ESI corporation data cache (corporation_id -> ESI Corporation).
    pub corporation_esi: HashMap<i64, Corporation>,
    /// Persisted corporation database models (corporation_id -> Model).
    pub corporation_model: HashMap<i64, EveCorporationModel>,
    /// Corporation database entry IDs (corporation_id -> database primary key).
    pub corporation_db_id: HashMap<i64, i32>,

    // Character data
    /// ESI character data cache (character_id -> ESI Character).
    pub character_esi: HashMap<i64, Character>,
    /// Persisted character database models (character_id -> Model).
    pub character_model: HashMap<i64, EveCharacterModel>,
    /// Character database entry IDs (character_id -> database primary key).
    pub character_db_id: HashMap<i64, i32>,

    // Transaction tracking - used to detect when a new transaction is being used
    /// Current transaction timestamp for detecting transaction retries and cache invalidation.
    transaction_id: Option<Instant>,
}

impl OrchestrationCache {
    /// Checks if the provided transaction ID differs from the cached one, and if so,
    /// clears all database model caches automatically.
    ///
    /// This enables automatic cache clearing on transaction retries without requiring
    /// explicit calls to clear the cache. Simply call this method at the start of each
    /// persistence operation, passing the transaction's creation timestamp.
    ///
    /// # Arguments
    /// - `txn_id` - Timestamp when the current transaction was created
    ///
    /// # Returns
    /// - `true` - Caches were cleared (new transaction detected)
    /// - `false` - Caches were not cleared (same transaction)
    ///
    /// # Note
    /// Only database model caches are cleared. ESI caches remain intact to avoid
    /// redundant API calls during retry attempts.
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

    /// Clears all database model caches manually.
    ///
    /// This is now primarily used internally by `check_and_clear_on_new_transaction`.
    /// You generally don't need to call this directly - instead use transaction tracking
    /// via [`TrackedTransaction`].
    ///
    /// # What Gets Cleared
    ///
    /// - All `*_model` caches (database models)
    /// - All `*_db_id` caches (EVE ID to database ID mappings)
    /// - Transaction ID tracking
    ///
    /// # What Remains
    ///
    /// - All `*_esi` caches (ESI data) are preserved to avoid redundant API calls
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

    /// Persists all ESI models from cache to the database in dependency order.
    ///
    /// This method persists all fetched ESI data in the cache to the database,
    /// respecting foreign key dependencies in the correct order:
    /// 1. Factions (no dependencies)
    /// 2. Alliances (depend on factions)
    /// 3. Corporations (depend on alliances and factions)
    /// 4. Characters (depend on corporations and factions)
    ///
    /// Only ESI models not already present in the database model cache will be persisted,
    /// ensuring idempotent behavior during retry attempts.
    ///
    /// # Arguments
    /// - `db` - Database connection for creating orchestrators
    /// - `esi_client` - ESI client for creating orchestrators
    /// - `txn` - Tracked database transaction to persist within
    ///
    /// # Returns
    /// - `Ok(())` - All entities were successfully persisted
    /// - `Err(Error)` - One or more persistence operations failed
    ///
    /// # Note
    /// This method automatically handles dependency resolution and will skip
    /// empty caches to avoid unnecessary database operations.
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

/// Extracts unique faction IDs that alliances depend on.
///
/// # Arguments
/// - `alliances` - Slice of alliance references to extract faction IDs from
///
/// # Returns
/// Vector of unique faction IDs (deduplicates multiple alliances with the same faction)
pub fn get_alliance_faction_dependency_ids(alliances: &[&Alliance]) -> Vec<i64> {
    alliances
        .iter()
        .filter_map(|a| a.faction_id)
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect::<Vec<i64>>()
}

/// Extracts unique faction IDs that corporations depend on.
///
/// # Arguments
/// - `corporations` - Slice of corporation references to extract faction IDs from
///
/// # Returns
/// Vector of unique faction IDs (deduplicates multiple corporations with the same faction)
pub fn get_corporation_faction_dependency_ids(corporations: &[&Corporation]) -> Vec<i64> {
    corporations
        .iter()
        .filter_map(|c| c.faction_id)
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect::<Vec<i64>>()
}

/// Extracts unique alliance IDs that corporations depend on.
///
/// # Arguments
/// - `corporations` - Slice of corporation references to extract alliance IDs from
///
/// # Returns
/// Vector of unique alliance IDs (deduplicates multiple corporations in the same alliance)
pub fn get_corporation_alliance_dependency_ids(corporations: &[&Corporation]) -> Vec<i64> {
    corporations
        .iter()
        .filter_map(|c| c.alliance_id)
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect::<Vec<i64>>()
}

/// Extracts unique faction IDs that characters depend on.
///
/// # Arguments
/// - `characters` - Slice of character references to extract faction IDs from
///
/// # Returns
/// Vector of unique faction IDs (deduplicates multiple characters with the same faction)
pub fn get_character_faction_dependency_ids(characters: &[&Character]) -> Vec<i64> {
    characters
        .iter()
        .filter_map(|c| c.faction_id)
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect::<Vec<i64>>()
}

/// Extracts unique corporation IDs that characters depend on.
///
/// # Arguments
/// - `characters` - Slice of character references to extract corporation IDs from
///
/// # Returns
/// Vector of unique corporation IDs (deduplicates multiple characters in the same corporation)
pub fn get_character_corporation_dependency_ids(characters: &[&Character]) -> Vec<i64> {
    characters
        .iter()
        .map(|c| c.corporation_id)
        .collect::<HashSet<i64>>()
        .into_iter()
        .collect::<Vec<i64>>()
}
