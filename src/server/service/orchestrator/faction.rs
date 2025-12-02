use std::collections::HashSet;

use chrono::Utc;
use eve_esi::model::universe::Faction;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::faction::FactionRepository,
    error::Error,
    service::orchestrator::{cache::TrackedTransaction, OrchestrationCache},
    util::time::effective_faction_cache_expiry,
};

/// Orchestrator for fetching and persisting EVE factions.
///
/// This orchestrator manages the complete lifecycle of EVE faction data including:
/// - Fetching faction information from ESI
/// - Persisting factions to the database
/// - Maintaining cache consistency across operations
/// - Managing ESI's 24-hour faction cache window
///
/// Factions have no foreign key dependencies on other entities, making them the base
/// of the dependency hierarchy for other orchestrators.
///
/// # ESI Cache Management
///
/// ESI caches faction data for 24 hours and resets at 11:05 UTC daily. This orchestrator
/// respects this cache window and avoids unnecessary fetches when stored factions are
/// still within the valid cache period.
///
/// # Example
///
/// ```ignore
/// let mut cache = OrchestrationCache::default();
/// let faction_orch = FactionOrchestrator::new(&db, &esi_client);
///
/// // Fetch all factions if cache is expired
/// if let Some(factions) = faction_orch.fetch_factions(&mut cache).await? {
///     // Persist within a transaction
///     let txn = TrackedTransaction::begin(&db).await?;
///     let models = faction_orch.persist_factions(&txn, factions, &mut cache).await?;
///     txn.commit().await?;
/// }
/// ```
pub struct FactionOrchestrator<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> FactionOrchestrator<'a> {
    /// Creates a new instance of [`FactionOrchestrator`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Retrieves the database entry ID for a faction by its EVE faction ID.
    ///
    /// This method first checks the cache, then queries the database if the ID is not cached.
    /// The result is cached for subsequent lookups.
    ///
    /// # Arguments
    /// - `faction_id` - EVE Online faction ID to look up
    /// - `cache` - Unified cache to prevent duplicate database queries
    ///
    /// # Returns
    /// - `Ok(Some(i32))` - Database entry ID if faction exists
    /// - `Ok(None)` - Faction does not exist in database
    /// - `Err(Error::DbErr)` - Database query failed
    pub async fn get_faction_record_id(
        &self,
        faction_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Option<i32>, Error> {
        let ids = self
            .get_many_faction_record_ids(vec![faction_id], cache)
            .await?;

        Ok(ids.into_iter().next().map(|(_, db_id)| db_id))
    }

    /// Retrieves database entry IDs for multiple factions by their EVE faction IDs.
    ///
    /// This method efficiently batches database lookups for factions not already in cache.
    /// Only missing IDs are queried from the database, and results are cached.
    ///
    /// # Arguments
    /// - `faction_ids` - List of EVE Online faction IDs to look up
    /// - `cache` - Unified cache to prevent duplicate database queries
    ///
    /// # Returns
    /// - `Ok(Vec<(i64, i32)>)` - Pairs of (EVE faction ID, database entry ID) for factions that exist
    /// - `Err(Error::DbErr)` - Database query failed
    ///
    /// # Note
    /// Only returns entries for factions that exist in the database. Missing factions are silently omitted.
    pub async fn get_many_faction_record_ids(
        &self,
        faction_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let missing_ids: Vec<i64> = faction_ids
            .iter()
            .filter(|id| !cache.faction_db_id.contains_key(id))
            .copied()
            .collect();

        if missing_ids.is_empty() {
            return Ok(faction_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .faction_db_id
                        .get(id)
                        .cloned()
                        .map(|db_id| (*id, db_id))
                })
                .collect());
        }

        let faction_repo = FactionRepository::new(self.db);

        let retrieved_ids = faction_repo
            .get_record_ids_by_faction_ids(&missing_ids)
            .await?;

        for (db_id, faction_id) in retrieved_ids {
            cache.faction_db_id.insert(faction_id, db_id);
        }

        Ok(faction_ids
            .iter()
            .filter_map(|id| {
                cache
                    .faction_db_id
                    .get(id)
                    .cloned()
                    .map(|db_id| (*id, db_id))
            })
            .collect())
    }

    /// Retrieves NPC faction information from ESI if the cache has expired.
    ///
    /// ESI only supports fetching all factions at once and caches faction information for
    /// 24 hours, resetting at 11:05 UTC daily. This method checks if the stored factions
    /// are within the valid cache window before fetching from ESI.
    ///
    /// # Arguments
    /// - `cache` - Unified cache for orchestration that prevents duplicate fetching of factions
    ///   during retry attempts
    ///
    /// # Returns
    /// - `Ok(Some(Vec<Faction>))` - Factions were fetched because cache expired
    /// - `Ok(None)` - Factions are already up to date, no fetch was performed
    /// - `Err(Error::EsiError)` - Failed to fetch factions from ESI
    /// - `Err(Error::DbErr)` - Failed to query database for latest faction timestamp
    ///
    /// # Note
    /// The effective cache expiry is calculated based on ESI's 24-hour cache window.
    /// Factions are automatically added to the cache when fetched.
    pub async fn fetch_factions(
        &self,
        cache: &mut OrchestrationCache,
    ) -> Result<Option<Vec<Faction>>, Error> {
        if !cache.faction_esi.is_empty() {
            return Ok(Some(cache.faction_esi.values().cloned().collect()));
        }

        // Check cache expiry against database
        let faction_repo = FactionRepository::new(self.db);

        let now = Utc::now();
        let effective_expiry = effective_faction_cache_expiry(now)?;

        // If the latest faction entry was updated at or after the effective expiry, return None
        // to prevent cache updates
        if let Some(latest_faction_model) = faction_repo.get_latest().await? {
            if latest_faction_model.updated_at >= effective_expiry {
                return Ok(None);
            }
        }

        // Fetch all factions from ESI
        let fetched_factions = self.esi_client.universe().get_factions().await?;

        for faction in &fetched_factions {
            cache
                .faction_esi
                .insert(faction.faction_id, faction.clone());
        }

        Ok(Some(fetched_factions))
    }

    /// Persists factions fetched from ESI to the database within a transaction.
    ///
    /// This method handles the complete persistence workflow:
    /// - Checks for new transactions and clears model caches if needed
    /// - Filters out factions already persisted in this transaction
    /// - Upserts factions to the database
    /// - Updates cache with persisted models
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `factions` - List of faction data from ESI to persist
    /// - `cache` - Unified cache for tracking persisted models
    ///
    /// # Returns
    /// - `Ok(Vec<Model>)` - Database models for the requested factions (cached + newly persisted)
    /// - `Err(Error::DbErr)` - Database operation failed
    ///
    /// # Note
    /// Factions have no dependencies, so this method can be called independently.
    pub async fn persist_factions(
        &self,
        txn: &TrackedTransaction,
        factions: Vec<Faction>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<entity::eve_faction::Model>, Error> {
        // Check if this is a new transaction and clear caches if needed
        cache.check_and_clear_on_new_transaction(txn.created_at);
        if factions.is_empty() {
            return Ok(Vec::new());
        }

        // Track which IDs were requested for return
        let requested_ids: std::collections::HashSet<i64> =
            factions.iter().map(|f| f.faction_id).collect();

        // Filter out factions that are already in the database model cache
        let factions_to_persist: Vec<Faction> = factions
            .into_iter()
            .filter(|faction| !cache.faction_model.contains_key(&faction.faction_id))
            .collect();

        if factions_to_persist.is_empty() {
            // Return only the models that were requested
            return Ok(cache
                .faction_model
                .iter()
                .filter(|(id, _)| requested_ids.contains(id))
                .map(|(_, model)| model.clone())
                .collect());
        }

        // Upsert factions to database
        let faction_repo = FactionRepository::new(txn.as_ref());
        let persisted_models = faction_repo.upsert_many(factions_to_persist).await?;

        for model in &persisted_models {
            cache.faction_model.insert(model.faction_id, model.clone());
            cache.faction_db_id.insert(model.faction_id, model.id);
        }

        // Return only the models that were requested (cached + newly persisted)
        Ok(cache
            .faction_model
            .iter()
            .filter(|(id, _)| requested_ids.contains(id))
            .map(|(_, model)| model.clone())
            .collect())
    }

    /// Ensures factions exist in the database, fetching from ESI if missing.
    ///
    /// This method checks the database for the provided faction IDs and fetches
    /// all factions from ESI if any are missing (since ESI only supports fetching
    /// all factions at once). Fetched factions are cached but not persisted.
    ///
    /// # Arguments
    /// - `faction_ids` - List of EVE Online faction IDs to verify/fetch
    /// - `cache` - Unified cache for tracking database entries and ESI data
    ///
    /// # Returns
    /// - `Ok(())` - All factions now exist in database or are cached for persistence
    /// - `Err(Error::EsiError)` - Failed to fetch factions from ESI
    /// - `Err(Error::DbErr)` - Database query failed
    ///
    /// # Note
    /// This method only ensures factions are fetched and cached. To persist them,
    /// use [`persist_cached_factions`](Self::persist_cached_factions) within a transaction.
    /// ESI fetches all factions at once, so even if only one faction is missing, all
    /// factions will be fetched and cached.
    pub async fn ensure_factions_exist(
        &self,
        faction_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<(), Error> {
        let existing_ids = self
            .get_many_faction_record_ids(faction_ids.clone(), cache)
            .await?;

        let existing_faction_ids: HashSet<i64> = existing_ids.iter().map(|(id, _)| *id).collect();

        let missing_ids: Vec<i64> = faction_ids
            .into_iter()
            .filter(|id| !existing_faction_ids.contains(id))
            .collect();

        if missing_ids.is_empty() {
            return Ok(());
        }

        // Fetch the factions if any IDs are missing
        self.fetch_factions(cache).await?;

        Ok(())
    }

    /// Persists all factions currently in the ESI cache to the database.
    ///
    /// This is a convenience method that persists all factions that have been fetched
    /// from ESI and are currently stored in the cache. Useful after calling methods like
    /// [`ensure_factions_exist`](Self::ensure_factions_exist).
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `cache` - Unified cache containing fetched factions
    ///
    /// # Returns
    /// - `Ok(Vec<Model>)` - Database models for all persisted factions
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn persist_cached_factions(
        &self,
        txn: &TrackedTransaction,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<entity::eve_faction::Model>, Error> {
        let factions: Vec<Faction> = cache.faction_esi.values().cloned().collect();
        self.persist_factions(txn, factions, cache).await
    }
}
