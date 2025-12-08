//! Corporation orchestrator for EVE Online corporation data operations.
//!
//! This module provides the `CorporationOrchestrator` for managing the complete lifecycle of
//! EVE corporation data including fetching from ESI, dependency resolution, and database persistence.
//! Corporations have optional foreign key dependencies on alliances and factions.

use std::collections::HashSet;

use dioxus_logger::tracing;
use eve_esi::model::corporation::Corporation;
use futures::stream::{FuturesUnordered, StreamExt};
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::corporation::CorporationRepository,
    error::Error,
    model::db::EveCorporationModel,
    service::orchestrator::{
        alliance::AllianceOrchestrator,
        cache::{
            get_corporation_alliance_dependency_ids, get_corporation_faction_dependency_ids,
            TrackedTransaction,
        },
        faction::FactionOrchestrator,
        OrchestrationCache,
    },
};

const MAX_CONCURRENT_CORPORATION_FETCHES: usize = 10;

/// Orchestrator for fetching and persisting EVE corporations and their dependencies.
///
/// This orchestrator manages the complete lifecycle of EVE corporation data including:
/// - Fetching corporation information from ESI
/// - Managing corporation dependencies (alliances, factions)
/// - Persisting corporations to the database
/// - Maintaining cache consistency across operations
///
/// Corporation data has optional foreign key dependencies on alliances and factions.
/// The orchestrator automatically ensures these dependencies exist before persisting corporations.
///
/// # Example
///
/// ```ignore
/// let mut cache = OrchestrationCache::default();
/// let corporation_orch = CorporationOrchestrator::new(&db, &esi_client);
///
/// // Fetch and cache a corporation
/// let corporation = corporation_orch.fetch_corporation(corporation_id, &mut cache).await?;
///
/// // Persist it within a transaction
/// let txn = TrackedTransaction::begin(&db).await?;
/// let model = corporation_orch.persist(&txn, corporation_id, corporation, &mut cache).await?;
/// txn.commit().await?;
/// ```
pub struct CorporationOrchestrator<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CorporationOrchestrator<'a> {
    /// Creates a new instance of CorporationOrchestrator.
    ///
    /// Constructs an orchestrator for managing EVE corporation data operations.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    /// - `esi_client` - ESI API client reference
    ///
    /// # Returns
    /// - `CorporationOrchestrator` - New orchestrator instance
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Retrieves the database entry ID for a corporation by its EVE corporation ID.
    ///
    /// This method first checks the cache, then queries the database if the ID is not cached.
    /// The result is cached for subsequent lookups.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID to look up
    /// - `cache` - Unified cache to prevent duplicate database queries
    ///
    /// # Returns
    /// - `Ok(Some(i32))` - Database entry ID if corporation exists
    /// - `Ok(None)` - Corporation does not exist in database
    /// - `Err(Error::DbErr)` - Database query failed
    pub async fn get_corporation_record_id(
        &self,
        corporation_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Option<i32>, Error> {
        let ids = self
            .get_many_corporation_record_ids(vec![corporation_id], cache)
            .await?;

        Ok(ids.into_iter().next().map(|(_, db_id)| db_id))
    }

    /// Retrieves database entry IDs for multiple corporations by their EVE corporation IDs.
    ///
    /// This method efficiently batches database lookups for corporations not already in cache.
    /// Only missing IDs are queried from the database, and results are cached.
    ///
    /// # Arguments
    /// - `corporation_ids` - List of EVE Online corporation IDs to look up
    /// - `cache` - Unified cache to prevent duplicate database queries
    ///
    /// # Returns
    /// - `Ok(Vec<(i64, i32)>)` - Pairs of (EVE corporation ID, database entry ID) for corporations that exist
    /// - `Err(Error::DbErr)` - Database query failed
    ///
    /// # Note
    /// Only returns entries for corporations that exist in the database. Missing corporations are silently omitted.
    pub async fn get_many_corporation_record_ids(
        &self,
        corporation_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let missing_ids: Vec<i64> = corporation_ids
            .iter()
            .filter(|id| !cache.corporation_db_id.contains_key(id))
            .copied()
            .collect();

        if missing_ids.is_empty() {
            return Ok(corporation_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .corporation_db_id
                        .get(id)
                        .cloned()
                        .map(|db_id| (*id, db_id))
                })
                .collect());
        }

        let corporation_repo = CorporationRepository::new(self.db);

        let retrieved_ids = corporation_repo
            .get_record_ids_by_corporation_ids(&missing_ids)
            .await?;

        for (db_id, corporation_id) in retrieved_ids {
            cache.corporation_db_id.insert(corporation_id, db_id);
        }

        Ok(corporation_ids
            .iter()
            .filter_map(|id| {
                cache
                    .corporation_db_id
                    .get(id)
                    .cloned()
                    .map(|db_id| (*id, db_id))
            })
            .collect())
    }

    /// Fetches a single corporation from ESI and ensures its dependencies exist.
    ///
    /// This method retrieves corporation information from ESI, caches it, and ensures that
    /// the corporation's alliance and faction (if any) exist in the database. Dependencies
    /// are fetched and cached if they don't already exist.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID to fetch
    /// - `cache` - Unified cache to store fetched data and prevent duplicate ESI calls
    ///
    /// # Returns
    /// - `Ok(Corporation)` - The fetched corporation data from ESI
    /// - `Err(Error::EsiError)` - Failed to fetch corporation from ESI
    /// - `Err(Error::DbErr)` - Failed to query database for dependencies
    ///
    /// # Note
    /// - The corporation is cached after fetching to avoid duplicate ESI calls during retries.
    /// - Dependencies (alliance, faction) are also fetched and cached if missing.
    pub async fn fetch_corporation(
        &self,
        corporation_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Corporation, Error> {
        // Return corporation if it was already fetched and exists in cache
        if let Some(corporation) = cache.corporation_esi.get(&corporation_id) {
            return Ok(corporation.clone());
        }

        // Fetch corporation information from ESI
        let fetched_corporation = self
            .esi_client
            .corporation()
            .get_corporation_information(corporation_id)
            .send()
            .await?
            .data;

        // Insert the fetched corporation into cache to avoid additional ESI fetches on retries
        cache
            .corporation_esi
            .insert(corporation_id, fetched_corporation.clone());

        // Ensure the corporation's alliance exists in database else fetch it for persistence later
        if let Some(alliance_id) = fetched_corporation.alliance_id {
            let alliance_orch = AllianceOrchestrator::new(self.db, self.esi_client);

            alliance_orch
                .ensure_alliances_exist(vec![alliance_id], cache)
                .await?;
        }

        // Ensure the corporation's faction exists in database else fetch it for persistence later
        if let Some(faction_id) = fetched_corporation.faction_id {
            let faction_orch = FactionOrchestrator::new(self.db, self.esi_client);

            faction_orch
                .ensure_factions_exist(vec![faction_id], cache)
                .await?;
        }

        Ok(fetched_corporation)
    }

    /// Fetches multiple corporations from ESI concurrently and ensures their dependencies exist.
    ///
    /// This method efficiently fetches multiple corporations by:
    /// - Checking cache first to avoid redundant ESI calls
    /// - Fetching missing corporations concurrently (up to MAX_CONCURRENT_CORPORATION_FETCHES at a time)
    /// - Automatically ensuring all dependencies (alliances, factions) exist
    ///
    /// # Arguments
    /// - `corporation_ids` - List of EVE Online corporation IDs to fetch
    /// - `cache` - Unified cache to store fetched data and prevent duplicate ESI calls
    ///
    /// # Returns
    /// - `Ok(Vec<(i64, Corporation)>)` - Pairs of (corporation ID, corporation data) for requested corporations
    /// - `Err(Error::EsiError)` - Failed to fetch one or more corporations from ESI
    /// - `Err(Error::DbErr)` - Failed to query database for dependencies
    ///
    /// # Note
    /// - All fetched corporations are cached.
    /// - The method ensures all alliance and faction dependencies exist before returning.
    pub async fn fetch_many_corporations(
        &self,
        corporation_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, Corporation)>, Error> {
        // Check which IDs are missing from cache
        let missing_ids: Vec<i64> = corporation_ids
            .iter()
            .filter(|id| !cache.corporation_esi.contains_key(id))
            .copied()
            .collect();

        // If no IDs are missing, return cached corporations
        if missing_ids.is_empty() {
            return Ok(corporation_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .corporation_esi
                        .get(id)
                        .map(|corporation| (*id, corporation.clone()))
                })
                .collect());
        }

        let mut fetched_corporations = Vec::new();

        for chunk in missing_ids.chunks(MAX_CONCURRENT_CORPORATION_FETCHES) {
            let mut futures = FuturesUnordered::new();
            let esi_client = self.esi_client;

            for &id in chunk {
                let future = async move {
                    let corporation = esi_client
                        .corporation()
                        .get_corporation_information(id)
                        .send()
                        .await?
                        .data;
                    Ok::<_, Error>((id, corporation))
                };
                futures.push(future);
            }

            while let Some(fetched_corporation) = futures.next().await {
                fetched_corporations.push(fetched_corporation?);
            }
        }

        for (corporation_id, corporation) in &fetched_corporations {
            cache
                .corporation_esi
                .insert(*corporation_id, corporation.clone());
        }

        let requested_corporations: Vec<(i64, Corporation)> = corporation_ids
            .iter()
            .filter_map(|id| {
                cache
                    .corporation_esi
                    .get(id)
                    .map(|corporation| (*id, corporation.clone()))
            })
            .collect();

        let corporations_ref: Vec<&Corporation> = requested_corporations
            .iter()
            .map(|(_, corporation)| corporation)
            .collect();

        let faction_ids = get_corporation_faction_dependency_ids(&corporations_ref);
        let alliance_ids = get_corporation_alliance_dependency_ids(&corporations_ref);

        if !faction_ids.is_empty() {
            let faction_orch = FactionOrchestrator::new(self.db, self.esi_client);
            faction_orch
                .ensure_factions_exist(faction_ids, cache)
                .await?;
        }

        if !alliance_ids.is_empty() {
            let alliance_orch = AllianceOrchestrator::new(self.db, self.esi_client);
            alliance_orch
                .ensure_alliances_exist(alliance_ids, cache)
                .await?;
        }

        Ok(requested_corporations)
    }

    /// Persists multiple corporations to the database within a transaction.
    ///
    /// This method handles the complete persistence workflow:
    /// - Checks for new transactions and clears model caches if needed
    /// - Filters out corporations already persisted in this transaction
    /// - Persists all dependencies (factions, alliances) first
    /// - Maps corporations to their dependency database IDs
    /// - Upserts corporations to the database
    /// - Updates cache with persisted models
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `corporations` - List of (corporation ID, corporation data) pairs to persist
    /// - `cache` - Unified cache for tracking persisted models and dependencies
    ///
    /// # Returns
    /// - `Ok(Vec<Model>)` - Database models for the requested corporations (cached + newly persisted)
    /// - `Err(Error::DbErr)` - Database operation failed
    ///
    /// # Note
    /// Optional alliance and faction dependencies that are missing will set their respective IDs to None with warnings.
    pub async fn persist_many(
        &self,
        txn: &TrackedTransaction,
        corporations: Vec<(i64, Corporation)>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<EveCorporationModel>, Error> {
        // Check if this is a new transaction and clear caches if needed
        cache.check_and_clear_on_new_transaction(txn.created_at);

        if corporations.is_empty() {
            return Ok(Vec::new());
        }

        // Track which IDs were requested for return
        let requested_ids: std::collections::HashSet<i64> =
            corporations.iter().map(|(id, _)| *id).collect();

        // Filter out corporations that are already in the database model cache
        let corporations_to_persist: Vec<(i64, Corporation)> = corporations
            .into_iter()
            .filter(|(corporation_id, _)| !cache.corporation_model.contains_key(corporation_id))
            .collect();

        if corporations_to_persist.is_empty() {
            // Return only the models that were requested
            return Ok(cache
                .corporation_model
                .iter()
                .filter(|(id, _)| requested_ids.contains(id))
                .map(|(_, model)| model.clone())
                .collect());
        }

        // Persist factions if any were fetched
        let faction_orch = FactionOrchestrator::new(self.db, self.esi_client);
        faction_orch.persist_cached_factions(txn, cache).await?;

        // Persist alliances if any were fetched
        let alliance_orch = AllianceOrchestrator::new(self.db, self.esi_client);
        alliance_orch.persist_cached_alliances(txn, cache).await?;

        let corporations_ref: Vec<&Corporation> = corporations_to_persist
            .iter()
            .map(|(_, corporation)| corporation)
            .collect();

        let faction_ids = get_corporation_faction_dependency_ids(&corporations_ref);
        let alliance_ids = get_corporation_alliance_dependency_ids(&corporations_ref);

        let faction_db_ids = faction_orch
            .get_many_faction_record_ids(faction_ids, cache)
            .await?;

        let alliance_db_ids = alliance_orch
            .get_many_alliance_record_ids(alliance_ids, cache)
            .await?;

        // Create a map of faction/alliance id -> db_id for easy lookup
        let faction_id_map: std::collections::HashMap<i64, i32> =
            faction_db_ids.into_iter().collect();
        let alliance_id_map: std::collections::HashMap<i64, i32> =
            alliance_db_ids.into_iter().collect();

        // Map corporations with their faction & alliance DB IDs
        let corporations_to_upsert: Vec<(i64, Corporation, Option<i32>, Option<i32>)> =
            corporations_to_persist
                .into_iter()
                .map(|(corporation_id, corporation)| {
                    let faction_db_id = corporation.faction_id.and_then(|faction_id| {
                        let db_id = faction_id_map.get(&faction_id).copied();
                        if db_id.is_none() {
                            tracing::warn!(
                                "Failed to find faction ID {} for corporation ID {}; \
                                setting corporation's faction ID to None for now",
                                faction_id,
                                corporation_id
                            )
                        }
                        db_id
                    });

                    let alliance_db_id = corporation.alliance_id.and_then(|alliance_id| {
                        let db_id = alliance_id_map.get(&alliance_id).copied();
                        if db_id.is_none() {
                            tracing::warn!(
                                "Failed to find alliance ID {} for corporation ID {}; \
                                setting corporation's alliance ID to None for now",
                                alliance_id,
                                corporation_id
                            )
                        }
                        db_id
                    });

                    (corporation_id, corporation, alliance_db_id, faction_db_id)
                })
                .collect();

        // Upsert corporations to database
        let corporation_repo = CorporationRepository::new(txn.as_ref());
        let persisted_corporations = corporation_repo.upsert_many(corporations_to_upsert).await?;

        for model in &persisted_corporations {
            cache
                .corporation_model
                .insert(model.corporation_id, model.clone());
            cache
                .corporation_db_id
                .insert(model.corporation_id, model.id);
        }

        // Return only the models that were requested (cached + newly persisted)
        Ok(cache
            .corporation_model
            .iter()
            .filter(|(id, _)| requested_ids.contains(id))
            .map(|(_, model)| model.clone())
            .collect())
    }

    /// Persists a single corporation to the database within a transaction.
    ///
    /// This is a convenience wrapper around [`persist_many`](Self::persist_many) for single corporation persistence.
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `corporation_id` - EVE Online corporation ID
    /// - `corporation` - Corporation data from ESI
    /// - `cache` - Unified cache for tracking persisted models and dependencies
    ///
    /// # Returns
    /// - `Ok(Model)` - Database model for the persisted corporation
    /// - `Err(Error::InternalError)` - Corporation persistence failed unexpectedly
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn persist(
        &self,
        txn: &TrackedTransaction,
        corporation_id: i64,
        corporation: Corporation,
        cache: &mut OrchestrationCache,
    ) -> Result<EveCorporationModel, Error> {
        // Delegate to persist_many with a single element
        let mut models = self
            .persist_many(txn, vec![(corporation_id, corporation)], cache)
            .await?;

        // Extract the single result - we expect exactly one since persist_many
        // returns one model per input corporation
        models.pop().ok_or_else(|| {
            Error::InternalError(format!(
                "Failed to persist corporation ID {} - expected one result but got none",
                corporation_id
            ))
        })
    }

    /// Ensures corporations exist in the database, fetching from ESI if missing.
    ///
    /// This method checks the database for the provided corporation IDs and fetches
    /// any missing corporations from ESI. Fetched corporations are cached but not persisted.
    ///
    /// # Arguments
    /// - `corporation_ids` - List of EVE Online corporation IDs to verify/fetch
    /// - `cache` - Unified cache for tracking database entries and ESI data
    ///
    /// # Returns
    /// - `Ok(())` - All corporations now exist in database or are cached for persistence
    /// - `Err(Error::EsiError)` - Failed to fetch missing corporations from ESI
    /// - `Err(Error::DbErr)` - Database query failed
    ///
    /// # Note
    /// This method only ensures corporations are fetched and cached. To persist them,
    /// use [`persist_cached_corporations`](Self::persist_cached_corporations) within a transaction.
    pub async fn ensure_corporations_exist(
        &self,
        corporation_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<(), Error> {
        let existing_ids = self
            .get_many_corporation_record_ids(corporation_ids.clone(), cache)
            .await?;

        let existing_corporation_ids: HashSet<i64> =
            existing_ids.iter().map(|(id, _)| *id).collect();

        let missing_ids: Vec<i64> = corporation_ids
            .into_iter()
            .filter(|id| !existing_corporation_ids.contains(id))
            .collect();

        if missing_ids.is_empty() {
            return Ok(());
        }

        // Fetch the corporations if any IDs are missing
        self.fetch_many_corporations(missing_ids, cache).await?;

        Ok(())
    }

    /// Persists all corporations currently in the ESI cache to the database.
    ///
    /// This is a convenience method that persists all corporations that have been fetched
    /// from ESI and are currently stored in the cache. Useful after calling methods like
    /// [`ensure_corporations_exist`](Self::ensure_corporations_exist).
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `cache` - Unified cache containing fetched corporations
    ///
    /// # Returns
    /// - `Ok(Vec<Model>)` - Database models for all persisted corporations
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn persist_cached_corporations(
        &self,
        txn: &TrackedTransaction,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<EveCorporationModel>, Error> {
        let corporations: Vec<(i64, Corporation)> = cache
            .corporation_esi
            .iter()
            .map(|(id, corporation)| (*id, corporation.clone()))
            .collect();
        self.persist_many(txn, corporations, cache).await
    }
}
