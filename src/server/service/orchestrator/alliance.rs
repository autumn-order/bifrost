//! Alliance orchestrator for EVE Online alliance data operations.
//!
//! This module provides the `AllianceOrchestrator` for managing the complete lifecycle of
//! EVE alliance data including fetching from ESI, dependency resolution, and database persistence.
//! Alliances have optional foreign key dependencies on factions.

use std::collections::HashSet;

use dioxus_logger::tracing;
use eve_esi::model::alliance::Alliance;
use futures::stream::{FuturesUnordered, StreamExt};
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::alliance::AllianceRepository,
    error::{eve::EveError, Error},
    model::db::EveAllianceModel,
    service::orchestrator::{
        cache::{get_alliance_faction_dependency_ids, TrackedTransaction},
        faction::FactionOrchestrator,
        OrchestrationCache,
    },
};

const MAX_CONCURRENT_ALLIANCE_FETCHES: usize = 10;

/// Orchestrator for fetching and persisting EVE alliances and their dependencies.
///
/// This orchestrator manages the complete lifecycle of EVE alliance data including:
/// - Fetching alliance information from ESI
/// - Managing alliance dependencies (factions)
/// - Persisting alliances to the database
///
/// Alliance data has an optional foreign key dependency on factions.
/// The orchestrator automatically ensures faction dependencies exist before persisting alliances.
///
/// # Example
///
/// ```ignore
/// let mut cache = OrchestrationCache::default();
/// let alliance_orch = AllianceOrchestrator::new(&db, &esi_client);
///
/// // Fetch and cache an alliance
/// let alliance = alliance_orch.fetch_alliance(alliance_id, &mut cache).await?;
///
/// // Persist it within a transaction
/// let txn = TrackedTransaction::begin(&db).await?;
/// let model = alliance_orch.persist(&txn, alliance_id, alliance, &mut cache).await?;
/// txn.commit().await?;
/// ```
pub struct AllianceOrchestrator<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> AllianceOrchestrator<'a> {
    /// Creates a new instance of AllianceOrchestrator.
    ///
    /// Constructs an orchestrator for managing EVE alliance data operations.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    /// - `esi_client` - ESI API client reference
    ///
    /// # Returns
    /// - `AllianceOrchestrator` - New orchestrator instance
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Retrieves the database entry ID for an alliance by its EVE alliance ID.
    ///
    /// This method first checks the cache, then queries the database if the ID is not cached.
    /// The result is cached for subsequent lookups.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID to look up
    /// - `cache` - Unified cache to prevent duplicate database queries
    ///
    /// # Returns
    /// - `Ok(Some(i32))` - Database entry ID if alliance exists
    /// - `Ok(None)` - Alliance does not exist in database
    /// - `Err(Error::DbErr)` - Database query failed
    pub async fn get_alliance_record_id(
        &self,
        alliance_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Option<i32>, Error> {
        let ids = self
            .get_many_alliance_record_ids(vec![alliance_id], cache)
            .await?;

        Ok(ids.into_iter().next().map(|(_, db_id)| db_id))
    }

    /// Retrieves database entry IDs for multiple alliances by their EVE alliance IDs.
    ///
    /// This method efficiently batches database lookups for alliances not already in cache.
    /// Only missing IDs are queried from the database, and results are cached.
    ///
    /// # Arguments
    /// - `alliance_ids` - List of EVE Online alliance IDs to look up
    /// - `cache` - Unified cache to prevent duplicate database queries
    ///
    /// # Returns
    /// - `Ok(Vec<(i64, i32)>)` - Pairs of (EVE alliance ID, database entry ID) for alliances that exist
    /// - `Err(Error::DbErr)` - Database query failed
    ///
    /// # Note
    /// Only returns entries for alliances that exist in the database. Missing alliances are silently omitted.
    pub async fn get_many_alliance_record_ids(
        &self,
        alliance_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let missing_ids: Vec<i64> = alliance_ids
            .iter()
            .filter(|id| !cache.alliance_db_id.contains_key(id))
            .copied()
            .collect();

        if missing_ids.is_empty() {
            return Ok(alliance_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .alliance_db_id
                        .get(id)
                        .cloned()
                        .map(|db_id| (*id, db_id))
                })
                .collect());
        }

        let alliance_repo = AllianceRepository::new(self.db);

        let retrieved_ids = alliance_repo
            .get_record_ids_by_alliance_ids(&missing_ids)
            .await?;

        for (db_id, alliance_id) in retrieved_ids {
            cache.alliance_db_id.insert(alliance_id, db_id);
        }

        Ok(alliance_ids
            .iter()
            .filter_map(|id| {
                cache
                    .alliance_db_id
                    .get(id)
                    .cloned()
                    .map(|db_id| (*id, db_id))
            })
            .collect())
    }

    /// Fetches a single alliance from ESI and ensures its dependencies exist.
    ///
    /// This method retrieves alliance information from ESI, caches it, and ensures that
    /// the alliance's faction (if any) exists in the database. Dependencies are fetched
    /// and cached if they don't already exist.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID to fetch
    /// - `cache` - Unified cache to store fetched data and prevent duplicate ESI calls
    ///
    /// # Returns
    /// - `Ok(Alliance)` - The fetched alliance data from ESI
    /// - `Err(Error::EsiError)` - Failed to fetch alliance from ESI
    /// - `Err(Error::DbErr)` - Failed to query database for dependencies
    ///
    /// # Note
    /// - The alliance is cached after fetching to avoid duplicate ESI calls during retries.
    /// - Dependencies (faction) are also fetched and cached if missing.
    pub async fn fetch_alliance(
        &self,
        alliance_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Alliance, Error> {
        // Return alliance if it was already fetched and exists in cache
        if let Some(alliance) = cache.alliance_esi.get(&alliance_id) {
            return Ok(alliance.clone());
        }

        // Fetch alliance information from ESI
        let fetched_alliance = self
            .esi_client
            .alliance()
            .get_alliance_information(alliance_id)
            .send()
            .await?
            .data;

        // Insert the fetched alliance into cache to avoid additional ESI fetches on retries
        cache
            .alliance_esi
            .insert(alliance_id, fetched_alliance.clone());

        // Ensure the alliance's faction exists in database else fetch it for persistence later
        if let Some(faction_id) = fetched_alliance.faction_id {
            let faction_orch = FactionOrchestrator::new(self.db, self.esi_client);

            faction_orch
                .ensure_factions_exist(vec![faction_id], cache)
                .await?;
        };

        Ok(fetched_alliance)
    }

    /// Fetches multiple alliances from ESI concurrently and ensures their dependencies exist.
    ///
    /// This method efficiently fetches multiple alliances by:
    /// - Checking cache first to avoid redundant ESI calls
    /// - Fetching missing alliances concurrently (up to MAX_CONCURRENT_ALLIANCE_FETCHES at a time)
    /// - Automatically ensuring all faction dependencies exist
    ///
    /// # Arguments
    /// - `alliance_ids` - Slice of EVE Online alliance IDs to fetch
    /// - `cache` - Unified cache to store fetched data and prevent duplicate ESI calls
    ///
    /// # Returns
    /// - `Ok(Vec<(i64, Alliance)>)` - Pairs of (alliance ID, alliance data) for requested alliances
    /// - `Err(Error::EsiError)` - Failed to fetch one or more alliances from ESI
    /// - `Err(Error::DbErr)` - Failed to query database for dependencies
    ///
    /// # Note
    /// - All fetched alliances are cached.
    /// - The method ensures all faction dependencies exist before returning.
    pub async fn fetch_many_alliances(
        &self,
        alliance_ids: &[i64],
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, Alliance)>, Error> {
        // Check which IDs are missing from cache
        let missing_ids: Vec<i64> = alliance_ids
            .iter()
            .filter(|id| !cache.alliance_esi.contains_key(id))
            .copied()
            .collect();

        // If no IDs are missing, return cached alliances
        if missing_ids.is_empty() {
            return Ok(alliance_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .alliance_esi
                        .get(id)
                        .map(|alliance| (*id, alliance.clone()))
                })
                .collect());
        }

        let mut fetched_alliances = Vec::new();

        for chunk in missing_ids.chunks(MAX_CONCURRENT_ALLIANCE_FETCHES) {
            let mut futures = FuturesUnordered::new();
            let esi_client = self.esi_client;

            for &id in chunk {
                let future = async move {
                    let alliance = esi_client
                        .alliance()
                        .get_alliance_information(id)
                        .send()
                        .await?
                        .data;
                    Ok::<_, Error>((id, alliance))
                };
                futures.push(future);
            }

            while let Some(fetched_alliance) = futures.next().await {
                fetched_alliances.push(fetched_alliance?);
            }
        }

        for (alliance_id, alliance) in &fetched_alliances {
            cache.alliance_esi.insert(*alliance_id, alliance.clone());
        }

        let requested_alliances: Vec<(i64, Alliance)> = alliance_ids
            .iter()
            .filter_map(|id| {
                cache
                    .alliance_esi
                    .get(id)
                    .map(|alliance| (*id, alliance.clone()))
            })
            .collect();

        let alliances_ref: Vec<&Alliance> = requested_alliances
            .iter()
            .map(|(_, alliance)| alliance)
            .collect();

        let faction_ids = get_alliance_faction_dependency_ids(&alliances_ref);

        if faction_ids.is_empty() {
            return Ok(requested_alliances);
        }

        // Ensure the faction IDs for the alliances exists in the database, else the faction
        // orchestrator will attempt to fetch updated factions from ESI if the factions
        // we have currently stored are out of date
        let faction_orch = FactionOrchestrator::new(self.db, self.esi_client);
        faction_orch
            .ensure_factions_exist(faction_ids, cache)
            .await?;

        Ok(requested_alliances)
    }

    /// Persists multiple alliances to the database within a transaction.
    ///
    /// This method handles the complete persistence workflow:
    /// - Checks for new transactions and clears model caches for retry if needed
    /// - Filters out alliances already persisted in this transaction
    /// - Persists all dependencies (factions) first
    /// - Maps alliances to their dependency database IDs
    /// - Upserts alliances to the database
    /// - Updates cache with persisted models
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `alliances` - List of (alliance ID, alliance data) pairs to persist
    /// - `cache` - Unified cache for tracking persisted models and dependencies
    ///
    /// # Returns
    /// - `Ok(Vec<Model>)` - Database models for the requested alliances (cached + newly persisted)
    /// - `Err(Error::DbErr)` - Database operation failed
    ///
    /// # Note
    /// - Optional faction dependencies that are missing will set the faction ID to None with a warning.
    pub async fn persist_many(
        &self,
        txn: &TrackedTransaction,
        alliances: Vec<(i64, Alliance)>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<EveAllianceModel>, Error> {
        // Check if this is a new transaction and clear caches if needed
        cache.check_and_clear_on_new_transaction(txn.created_at);
        if alliances.is_empty() {
            return Ok(Vec::new());
        }

        // Track which IDs were requested for return
        let requested_ids: std::collections::HashSet<i64> =
            alliances.iter().map(|(id, _)| *id).collect();

        // Filter out alliances that are already in the database model cache
        let alliances_to_persist: Vec<(i64, Alliance)> = alliances
            .into_iter()
            .filter(|(alliance_id, _)| !cache.alliance_model.contains_key(alliance_id))
            .collect();

        if alliances_to_persist.is_empty() {
            // Return only the models that were requested
            return Ok(cache
                .alliance_model
                .iter()
                .filter(|(id, _)| requested_ids.contains(id))
                .map(|(_, model)| model.clone())
                .collect());
        }

        // Persist factions if any were fetched
        let faction_orch = FactionOrchestrator::new(self.db, self.esi_client);
        faction_orch.persist_cached_factions(txn, cache).await?;

        // Get the DB IDs for factions to map to alliances
        let alliances_ref: Vec<&Alliance> = alliances_to_persist
            .iter()
            .map(|(_, alliance)| alliance)
            .collect();

        let faction_ids = get_alliance_faction_dependency_ids(&alliances_ref);

        let faction_db_ids = faction_orch
            .get_many_faction_record_ids(faction_ids, cache)
            .await?;

        // Create a map of faction_id -> db_id for easy lookup
        let faction_id_map: std::collections::HashMap<i64, i32> =
            faction_db_ids.into_iter().collect();

        // Map alliances with their faction DB IDs
        let alliances_to_upsert: Vec<(i64, Alliance, Option<i32>)> = alliances_to_persist
            .into_iter()
            .map(|(alliance_id, alliance)| {
                let faction_db_id = alliance.faction_id.and_then(|faction_id| {
                    let db_id = faction_id_map.get(&faction_id).copied();
                    if db_id.is_none() {
                        tracing::warn!("{}", EveError::FactionNotFound(faction_id));
                    }
                    db_id
                });
                (alliance_id, alliance, faction_db_id)
            })
            .collect();

        // Upsert alliances to database
        let alliance_repo = AllianceRepository::new(txn.as_ref());
        let persisted_alliances = alliance_repo.upsert_many(alliances_to_upsert).await?;

        for model in &persisted_alliances {
            cache
                .alliance_model
                .insert(model.alliance_id, model.clone());
            cache.alliance_db_id.insert(model.alliance_id, model.id);
        }

        // Return only the models that were requested (cached + newly persisted)
        Ok(cache
            .alliance_model
            .iter()
            .filter(|(id, _)| requested_ids.contains(id))
            .map(|(_, model)| model.clone())
            .collect())
    }

    /// Persists a single alliance to the database within a transaction.
    ///
    /// This is a convenience wrapper around [`persist_many`](Self::persist_many) for single alliance persistence.
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `alliance_id` - EVE Online alliance ID
    /// - `alliance` - Alliance data from ESI
    /// - `cache` - Unified cache for tracking persisted models and dependencies
    ///
    /// # Returns
    /// - `Ok(Model)` - Database model for the persisted alliance
    /// - `Err(Error::InternalError)` - Alliance persistence failed unexpectedly
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn persist(
        &self,
        txn: &TrackedTransaction,
        alliance_id: i64,
        alliance: Alliance,
        cache: &mut OrchestrationCache,
    ) -> Result<EveAllianceModel, Error> {
        // Delegate to persist_many with a single element
        let mut models = self
            .persist_many(txn, vec![(alliance_id, alliance)], cache)
            .await?;

        // Extract the single result - we expect exactly one since persist_many
        // returns one model per input alliance
        models.pop().ok_or_else(|| {
            Error::InternalError(format!(
                "Failed to persist alliance ID {} - expected one result but got none",
                alliance_id
            ))
        })
    }

    /// Ensures alliances exist in the database, fetching from ESI if missing.
    ///
    /// This method checks the database for the provided alliance IDs and fetches
    /// any missing alliances from ESI. Fetched alliances are cached but not persisted.
    ///
    /// # Arguments
    /// - `alliance_ids` - List of EVE Online alliance IDs to verify/fetch
    /// - `cache` - Unified cache for tracking database entries and ESI data
    ///
    /// # Returns
    /// - `Ok(())` - All alliances exist in database or are now cached for persistence
    /// - `Err(Error::EsiError)` - Failed to fetch missing alliances from ESI
    /// - `Err(Error::DbErr)` - Database query failed
    ///
    /// # Note
    /// This method only ensures alliances are fetched and cached. To persist them,
    /// use [`persist_cached_alliances`](Self::persist_cached_alliances) within a transaction.
    pub async fn ensure_alliances_exist(
        &self,
        alliance_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<(), Error> {
        let existing_ids = self
            .get_many_alliance_record_ids(alliance_ids.clone(), cache)
            .await?;

        let existing_alliance_ids: HashSet<i64> = existing_ids.iter().map(|(id, _)| *id).collect();

        let missing_ids: Vec<i64> = alliance_ids
            .into_iter()
            .filter(|id| !existing_alliance_ids.contains(id))
            .collect();

        if missing_ids.is_empty() {
            return Ok(());
        }

        // Fetch the alliances if any IDs are missing
        self.fetch_many_alliances(&missing_ids, cache).await?;

        Ok(())
    }

    /// Persists all alliances currently in the ESI cache to the database.
    ///
    /// This is a convenience method that persists all alliances that have been fetched
    /// from ESI and are currently stored in the cache. Useful after calling methods like
    /// [`ensure_alliances_exist`](Self::ensure_alliances_exist).
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `cache` - Unified cache containing fetched alliances
    ///
    /// # Returns
    /// - `Ok(Vec<Model>)` - Database models for all persisted alliances
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn persist_cached_alliances(
        &self,
        txn: &TrackedTransaction,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<EveAllianceModel>, Error> {
        let alliances: Vec<(i64, Alliance)> = cache
            .alliance_esi
            .iter()
            .map(|(id, alliance)| (*id, alliance.clone()))
            .collect();
        self.persist_many(txn, alliances, cache).await
    }
}
