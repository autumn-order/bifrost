//! Character orchestrator for EVE Online character data operations.
//!
//! This module provides the `CharacterOrchestrator` for managing the complete lifecycle of
//! EVE character data including fetching from ESI, dependency resolution, and database persistence.
//! Characters have required foreign key dependencies on corporations and optional dependencies on factions.

use std::collections::HashSet;

use dioxus_logger::tracing;
use eve_esi::model::character::Character;
use futures::stream::{FuturesUnordered, StreamExt};
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::character::CharacterRepository,
    error::Error,
    service::orchestrator::{
        cache::{
            get_character_corporation_dependency_ids, get_character_faction_dependency_ids,
            TrackedTransaction,
        },
        corporation::CorporationOrchestrator,
        faction::FactionOrchestrator,
        OrchestrationCache,
    },
};

const MAX_CONCURRENT_CHARACTER_FETCHES: usize = 10;

/// Orchestrator for fetching and persisting EVE characters and their dependencies.
///
/// This orchestrator manages the complete lifecycle of EVE character data including:
/// - Fetching character information from ESI
/// - Managing character dependencies (corporations, factions)
/// - Persisting characters to the database
/// - Maintaining cache consistency across operations
///
/// Character data has foreign key dependencies on corporations (required) and factions (optional).
/// The orchestrator automatically ensures these dependencies exist before persisting characters.
///
/// # Example
///
/// ```ignore
/// let mut cache = OrchestrationCache::default();
/// let character_orch = CharacterOrchestrator::new(&db, &esi_client);
///
/// // Fetch and cache a character
/// let character = character_orch.fetch_character(character_id, &mut cache).await?;
///
/// // Persist it within a transaction
/// let txn = TrackedTransaction::begin(&db).await?;
/// let model = character_orch.persist(&txn, character_id, character, &mut cache).await?;
/// txn.commit().await?;
/// ```
pub struct CharacterOrchestrator<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CharacterOrchestrator<'a> {
    /// Creates a new instance of CharacterOrchestrator.
    ///
    /// Constructs an orchestrator for managing EVE character data operations.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    /// - `esi_client` - ESI API client reference
    ///
    /// # Returns
    /// - `CharacterOrchestrator` - New orchestrator instance
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Retrieves the database entry ID for a character by its EVE character ID.
    ///
    /// This method first checks the cache, then queries the database if the ID is not cached.
    /// The result is cached for subsequent lookups.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID to look up
    /// - `cache` - Unified cache to prevent duplicate database queries
    ///
    /// # Returns
    /// - `Ok(Some(i32))` - Database entry ID if character exists
    /// - `Ok(None)` - Character does not exist in database
    /// - `Err(Error::DbErr)` - Database query failed
    pub async fn get_character_record_id(
        &self,
        character_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Option<i32>, Error> {
        let ids = self
            .get_many_character_record_ids(vec![character_id], cache)
            .await?;

        Ok(ids.into_iter().next().map(|(_, db_id)| db_id))
    }

    /// Retrieves database entry IDs for multiple characters by their EVE character IDs.
    ///
    /// This method efficiently batches database lookups for characters not already in cache.
    /// Only missing IDs are queried from the database, and results are cached.
    ///
    /// # Arguments
    /// - `character_ids` - List of EVE Online character IDs to look up
    /// - `cache` - Unified cache to prevent duplicate database queries
    ///
    /// # Returns
    /// - `Ok(Vec<(i64, i32)>)` - Pairs of (EVE character ID, database entry ID) for characters that exist
    /// - `Err(Error::DbErr)` - Database query failed
    ///
    /// # Note
    /// - Only returns entries for characters that exist in the database.
    /// - Missing characters are silently omitted.
    pub async fn get_many_character_record_ids(
        &self,
        character_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let missing_ids: Vec<i64> = character_ids
            .iter()
            .filter(|id| !cache.character_db_id.contains_key(id))
            .copied()
            .collect();

        if missing_ids.is_empty() {
            return Ok(character_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .character_db_id
                        .get(id)
                        .cloned()
                        .map(|db_id| (*id, db_id))
                })
                .collect());
        }

        let character_repo = CharacterRepository::new(self.db);

        let retrieved_ids = character_repo
            .get_record_ids_by_character_ids(&missing_ids)
            .await?;

        for (db_id, character_id) in retrieved_ids {
            cache.character_db_id.insert(character_id, db_id);
        }

        Ok(character_ids
            .iter()
            .filter_map(|id| {
                cache
                    .character_db_id
                    .get(id)
                    .cloned()
                    .map(|db_id| (*id, db_id))
            })
            .collect())
    }

    /// Fetches a single character from ESI and ensures its dependencies exist.
    ///
    /// This method retrieves character information from ESI, caches it, and ensures that
    /// the character's corporation and faction (if any) exist in the database. Dependencies
    /// are fetched and cached if they don't already exist.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID to fetch
    /// - `cache` - Unified cache to store fetched data and prevent duplicate ESI calls
    ///
    /// # Returns
    /// - `Ok(Character)` - The fetched character data from ESI
    /// - `Err(Error::EsiError)` - Failed to fetch character from ESI
    /// - `Err(Error::DbErr)` - Failed to query database for dependencies
    ///
    /// # Note
    /// - The character is cached after fetching to avoid duplicate ESI calls during retries.
    /// - Dependencies (corporation, faction) are also fetched and cached if missing.
    pub async fn fetch_character(
        &self,
        character_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Character, Error> {
        // Return character if it was already fetched and exists in cache
        if let Some(character) = cache.character_esi.get(&character_id) {
            return Ok(character.clone());
        }

        // Fetch character information from ESI
        let fetched_character = self
            .esi_client
            .character()
            .get_character_public_information(character_id)
            .await?;

        // Insert the fetched character into cache to avoid additional ESI fetches on retries
        cache
            .character_esi
            .insert(character_id, fetched_character.clone());

        // Ensure the character's corporation exists in database else fetch it for persistence later
        let corporation_orch = CorporationOrchestrator::new(&self.db, &self.esi_client);
        corporation_orch
            .ensure_corporations_exist(vec![fetched_character.corporation_id], cache)
            .await?;

        // Ensure the character's faction exists in database else fetch it for persistence later
        if let Some(faction_id) = fetched_character.faction_id {
            let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);

            faction_orch
                .ensure_factions_exist(vec![faction_id], cache)
                .await?;
        }

        Ok(fetched_character)
    }

    /// Fetches multiple characters from ESI concurrently and ensures their dependencies exist.
    ///
    /// This method efficiently fetches multiple characters by:
    /// - Checking cache first to avoid redundant ESI calls
    /// - Fetching missing characters concurrently (up to MAX_CONCURRENT_CHARACTER_FETCHES at a time)
    /// - Automatically ensuring all dependencies (corporations, factions) exist
    ///
    /// # Arguments
    /// - `character_ids` - List of EVE Online character IDs to fetch
    /// - `cache` - Unified cache to store fetched data and prevent duplicate ESI calls
    ///
    /// # Returns
    /// - `Ok(Vec<(i64, Character)>)` - Pairs of (character ID, character data) for requested characters
    /// - `Err(Error::EsiError)` - Failed to fetch one or more characters from ESI
    /// - `Err(Error::DbErr)` - Failed to query database for dependencies
    ///
    /// # Note
    /// - All fetched characters are cached.
    /// - The method ensures all corporation and faction dependencies exist before returning.
    pub async fn fetch_many_characters(
        &self,
        character_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, Character)>, Error> {
        // Check which IDs are missing from cache
        let missing_ids: Vec<i64> = character_ids
            .iter()
            .filter(|id| !cache.character_esi.contains_key(id))
            .copied()
            .collect();

        // If no IDs are missing, return cached characters
        if missing_ids.is_empty() {
            return Ok(character_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .character_esi
                        .get(id)
                        .map(|character| (*id, character.clone()))
                })
                .collect());
        }

        let mut fetched_characters = Vec::new();

        for chunk in missing_ids.chunks(MAX_CONCURRENT_CHARACTER_FETCHES) {
            let mut futures = FuturesUnordered::new();
            let esi_client = self.esi_client;

            for &id in chunk {
                let future = async move {
                    let character = esi_client
                        .character()
                        .get_character_public_information(id)
                        .await?;
                    Ok::<_, Error>((id, character))
                };
                futures.push(future);
            }

            while let Some(fetched_character) = futures.next().await {
                fetched_characters.push(fetched_character?);
            }
        }

        for (character_id, character) in &fetched_characters {
            cache.character_esi.insert(*character_id, character.clone());
        }

        let requested_characters: Vec<(i64, Character)> = character_ids
            .iter()
            .filter_map(|id| {
                cache
                    .character_esi
                    .get(id)
                    .map(|character| (*id, character.clone()))
            })
            .collect();

        let characters_ref: Vec<&Character> = requested_characters
            .iter()
            .map(|(_, character)| character)
            .collect();

        let faction_ids = get_character_faction_dependency_ids(&characters_ref);
        let corporation_ids = get_character_corporation_dependency_ids(&characters_ref);

        if !faction_ids.is_empty() {
            let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);
            faction_orch
                .ensure_factions_exist(faction_ids, cache)
                .await?;
        }

        if !corporation_ids.is_empty() {
            let corporation_orch = CorporationOrchestrator::new(&self.db, &self.esi_client);
            corporation_orch
                .ensure_corporations_exist(corporation_ids, cache)
                .await?;
        }

        Ok(requested_characters)
    }

    /// Persists multiple characters to the database within a transaction.
    ///
    /// This method handles the complete persistence workflow:
    /// - Checks for new transactions and clears model caches if needed
    /// - Filters out characters already persisted in this transaction
    /// - Persists all dependencies (factions, corporations) first
    /// - Maps characters to their dependency database IDs
    /// - Upserts characters to the database
    /// - Updates cache with persisted models
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `characters` - List of (character ID, character data) pairs to persist
    /// - `cache` - Unified cache for tracking persisted models and dependencies
    ///
    /// # Returns
    /// - `Ok(Vec<Model>)` - Database models for the requested characters (cached + newly persisted)
    /// - `Err(Error::DbErr)` - Database operation failed
    ///
    /// # Note
    /// - Characters missing their required corporation dependency will be skipped with a warning.
    /// - Optional faction dependencies that are missing will set the faction ID to None with a warning.
    pub async fn persist_many(
        &self,
        txn: &TrackedTransaction,
        characters: Vec<(i64, Character)>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<entity::eve_character::Model>, Error> {
        // Check if this is a new transaction and clear caches if needed
        cache.check_and_clear_on_new_transaction(txn.created_at);
        if characters.is_empty() {
            return Ok(Vec::new());
        }

        // Track which IDs were requested for return
        let requested_ids: std::collections::HashSet<i64> =
            characters.iter().map(|(id, _)| *id).collect();

        // Filter out characters that are already in the database model cache
        let characters_to_persist: Vec<(i64, Character)> = characters
            .into_iter()
            .filter(|(character_id, _)| !cache.character_model.contains_key(character_id))
            .collect();

        if characters_to_persist.is_empty() {
            // Return only the models that were requested
            return Ok(cache
                .character_model
                .iter()
                .filter(|(id, _)| requested_ids.contains(id))
                .map(|(_, model)| model.clone())
                .collect());
        }

        // Persist factions if any were fetched
        let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);
        faction_orch.persist_cached_factions(txn, cache).await?;

        // Persist corporations if any were fetched
        let corporation_orch = CorporationOrchestrator::new(&self.db, &self.esi_client);
        corporation_orch
            .persist_cached_corporations(txn, cache)
            .await?;

        let characters_ref: Vec<&Character> = characters_to_persist
            .iter()
            .map(|(_, character)| character)
            .collect();

        let faction_ids = get_character_faction_dependency_ids(&characters_ref);
        let corporation_ids = get_character_corporation_dependency_ids(&characters_ref);

        let faction_db_ids = faction_orch
            .get_many_faction_record_ids(faction_ids, cache)
            .await?;

        let corporation_db_ids = corporation_orch
            .get_many_corporation_record_ids(corporation_ids, cache)
            .await?;

        // Create a map of faction/corporation id -> db_id for easy lookup
        let faction_id_map: std::collections::HashMap<i64, i32> =
            faction_db_ids.into_iter().collect();
        let corporation_id_map: std::collections::HashMap<i64, i32> =
            corporation_db_ids.into_iter().collect();

        // Map characters with their faction & corporation DB IDs
        let characters_to_upsert: Vec<(i64, Character, i32, Option<i32>)> = characters_to_persist
            .into_iter()
            .filter_map(|(character_id, character)| {
                let faction_db_id = character.faction_id.and_then(|faction_id| {
                    let db_id = faction_id_map.get(&faction_id).copied();
                    if db_id.is_none() {
                        tracing::warn!(
                            "Failed to find faction ID {} for character ID {}; \
                            setting character's faction ID to None for now",
                            faction_id,
                            character_id
                        );
                    }
                    db_id
                });

                let corporation_db_id = corporation_id_map.get(&character.corporation_id).copied();
                if corporation_db_id.is_none() {
                    tracing::error!(
                        "Failed to find corporation ID {} for character ID {}; \
                        skipping character persistence",
                        character.corporation_id,
                        character_id
                    );
                    return None;
                }

                Some((
                    character_id,
                    character,
                    corporation_db_id.unwrap(),
                    faction_db_id,
                ))
            })
            .collect();

        // Upsert characters to database
        let character_repo = CharacterRepository::new(txn.as_ref());
        let persisted_characters = character_repo.upsert_many(characters_to_upsert).await?;

        for model in &persisted_characters {
            cache
                .character_model
                .insert(model.character_id, model.clone());
            cache.character_db_id.insert(model.character_id, model.id);
        }

        // Return only the models that were requested (cached + newly persisted)
        Ok(cache
            .character_model
            .iter()
            .filter(|(id, _)| requested_ids.contains(id))
            .map(|(_, model)| model.clone())
            .collect())
    }

    /// Persists a single character to the database within a transaction.
    ///
    /// This is a convenience wrapper around [`persist_many`](Self::persist_many) for single character persistence.
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `character_id` - EVE Online character ID
    /// - `character` - Character data from ESI
    /// - `cache` - Unified cache for tracking persisted models and dependencies
    ///
    /// # Returns
    /// - `Ok(Model)` - Database model for the persisted character
    /// - `Err(Error::InternalError)` - Character persistence failed (likely missing corporation dependency)
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn persist(
        &self,
        txn: &TrackedTransaction,
        character_id: i64,
        character: Character,
        cache: &mut OrchestrationCache,
    ) -> Result<entity::eve_character::Model, Error> {
        // Delegate to persist_many with a single element
        let mut models = self
            .persist_many(txn, vec![(character_id, character)], cache)
            .await?;

        // Extract the single result - note that persist_many uses filter_map
        // and will skip characters if their required corporation dependency is missing
        models.pop().ok_or_else(|| {
            Error::InternalError(format!(
                "Failed to persist character ID {} - likely missing corporation dependency",
                character_id
            ))
        })
    }

    /// Ensures characters exist in the database, fetching from ESI if missing.
    ///
    /// This method checks the database for the provided character IDs and fetches
    /// any missing characters from ESI. Fetched characters are cached but not persisted.
    ///
    /// # Arguments
    /// - `character_ids` - List of EVE Online character IDs to verify/fetch
    /// - `cache` - Unified cache for tracking database entries and ESI data
    ///
    /// # Returns
    /// - `Ok(())` - All characters now exist in database or are cached for persistence
    /// - `Err(Error::EsiError)` - Failed to fetch missing characters from ESI
    /// - `Err(Error::DbErr)` - Database query failed
    ///
    /// # Note
    /// This method only ensures characters are fetched and cached. To persist them,
    /// use [`persist_cached_characters`](Self::persist_cached_characters) within a transaction.
    pub async fn ensure_characters_exist(
        &self,
        character_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<(), Error> {
        let existing_ids = self
            .get_many_character_record_ids(character_ids.clone(), cache)
            .await?;

        let existing_character_ids: HashSet<i64> = existing_ids.iter().map(|(id, _)| *id).collect();

        let missing_ids: Vec<i64> = character_ids
            .into_iter()
            .filter(|id| !existing_character_ids.contains(id))
            .collect();

        if missing_ids.is_empty() {
            return Ok(());
        }

        // Fetch the characters if any IDs are missing
        self.fetch_many_characters(missing_ids, cache).await?;

        Ok(())
    }

    /// Persists all characters currently in the ESI cache to the database.
    ///
    /// This is a convenience method that persists all characters that have been fetched
    /// from ESI and are currently stored in the cache. Useful after calling methods like
    /// [`ensure_characters_exist`](Self::ensure_characters_exist).
    ///
    /// # Arguments
    /// - `txn` - Tracked database transaction to persist within
    /// - `cache` - Unified cache containing fetched characters
    ///
    /// # Returns
    /// - `Ok(Vec<Model>)` - Database models for all persisted characters
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn persist_cached_characters(
        &self,
        txn: &TrackedTransaction,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<entity::eve_character::Model>, Error> {
        let characters: Vec<(i64, Character)> = cache
            .character_esi
            .iter()
            .map(|(id, character)| (*id, character.clone()))
            .collect();
        self.persist_many(txn, characters, cache).await
    }
}
