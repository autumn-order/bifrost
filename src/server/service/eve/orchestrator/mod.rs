//! # EVE Online Entity Orchestrator
//!
//! This module provides a two-phase system for fetching EVE Online entities from ESI
//! and persisting them to the database with proper relationship handling.
//!
//! ## Overview
//!
//! The orchestrator system consists of two main components:
//!
//! - [`EveEntityOrchestratorBuilder`]: Fetches entities from ESI and resolves database relationships
//! - [`EveEntityOrchestrator`]: Stores fetched entities to the database with relationship integrity
//!
//! ## Workflow
//!
//! 1. **Build Phase**: Request entities by ID, the builder fetches from ESI and resolves dependencies
//! 2. **Store Phase**: Persist all entities to database in dependency order (factions → alliances → corporations → characters)
//!
//! ## Relationship Handling
//!
//! The provider automatically:
//! - Fetches related entities (e.g., a character's corporation, corporation's alliance)
//! - Checks the database for existing related entities to avoid redundant ESI calls
//! - Stores entities in dependency order to maintain referential integrity
//! - Logs warnings when related entities cannot be found
//!
//! ## Example
//!
/// ```no_run
/// use bifrost::server::{service::eve::orchestrator::EveEntityOrchestrator, error::Error};
/// use sea_orm::{DatabaseConnection, TransactionTrait};
///
/// async fn update_character_affiliations(
///     db: &DatabaseConnection,
///     esi_client: &eve_esi::Client,
///     character_ids: Vec<i64>,
/// ) -> Result<(), Error> {
///     let txn = db.begin().await?;
///
///     let orchestrator = EveEntityOrchestrator::builder(db, esi_client)
///         .characters(character_ids)
///         .build()
///         .await?;
///
///     let stored = orchestrator.store(&txn).await?;
///     txn.commit().await?;
///
///     Ok(())
/// }
/// ```
mod builder;
mod stored;
mod util;

use std::collections::HashMap;

use dioxus_logger::tracing;
use entity::{
    eve_alliance::Model as EveAllianceModel, eve_character::Model as EveCharacterModel,
    eve_corporation::Model as EveCorporationModel, eve_faction::Model as EveFactionModel,
};
use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction};

use crate::server::{
    data::eve::{
        alliance::AllianceRepository, character::CharacterRepository,
        corporation::CorporationRepository, faction::FactionRepository,
    },
    error::Error,
};

pub use builder::EveEntityOrchestratorBuilder;
pub use stored::StoredEntities;

/// Result of fetching factions from ESI.
///
/// This enum prevents illegal states by ensuring factions can only be in one of four valid states:
/// - Fresh data was fetched and needs to be stored
/// - ESI returned 304 Not Modified (timestamp update needed)
/// - Database factions are still fresh (load from DB without updating)
/// - No factions were requested (skip entirely)
pub(super) enum FactionFetchState {
    /// Fresh faction data was fetched from ESI and should be stored
    Fresh(HashMap<i64, Faction>),
    /// ESI returned 304 Not Modified - only timestamp update needed
    NotModified,
    /// Database factions are still fresh - load from DB without updating
    UpToDate,
    /// No factions were requested - skip entirely
    NotRequested,
}

/// Orchestrates EVE Entities fetched from ESI via the [`EveEntityOrchestratorBuilder`].
///
/// Contains fetched ESI data and database relationship mappings needed to store
/// entities with proper foreign key references.
///
/// # Usage
///
/// After building, call [`store()`](Self::store) within a database transaction to persist all entities.
/// The orchestrator consumes itself during storage to prevent reuse.
///
/// # Relationship Integrity
///
/// Entities are stored in dependency order:
/// 1. Factions (if any were fetched)
/// 2. Alliances (references factions)
/// 3. Corporations (references alliances and factions)
/// 4. Characters (references corporations and factions)
pub struct EveEntityOrchestrator {
    // Entities fetched from ESI
    factions: FactionFetchState,
    alliances_map: HashMap<i64, Alliance>,
    corporations_map: HashMap<i64, Corporation>,
    characters_map: HashMap<i64, Character>,

    // Maps EVE ID -> DB Record ID
    // Tracks database record IDs for entities that already existed in the database
    // before fetching, used to establish foreign key relationships during storage
    factions_record_id_map: HashMap<i64, i32>,
    alliances_record_id_map: HashMap<i64, i32>,
    corporations_record_id_map: HashMap<i64, i32>,
    characters_record_id_map: HashMap<i64, i32>,
}

impl EveEntityOrchestrator {
    /// Creates a new builder for fetching and constructing an EVE entity orchestrator.
    ///
    /// This is the primary way to construct an `EveEntityOrchestrator`.
    pub fn builder<'a>(
        db: &'a DatabaseConnection,
        esi_client: &'a eve_esi::Client,
    ) -> EveEntityOrchestratorBuilder<'a> {
        EveEntityOrchestratorBuilder::new(db, esi_client)
    }

    /// Gets a character from the fetched ESI data by character ID.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID
    ///
    /// # Returns
    /// - `Some(&Character)` - Character data if it was requested and fetched
    /// - `None` - Character was not requested in the builder
    pub fn get_character(&self, character_id: i64) -> Option<&Character> {
        self.characters_map.get(&character_id)
    }

    /// Gets a corporation from the fetched ESI data by corporation ID.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID
    ///
    /// # Returns
    /// - `Some(&Corporation)` - Corporation data if it was requested and fetched
    /// - `None` - Corporation was not requested in the builder
    pub fn get_corporation(&self, corporation_id: i64) -> Option<&Corporation> {
        self.corporations_map.get(&corporation_id)
    }

    /// Gets an alliance from the fetched ESI data by alliance ID.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID
    ///
    /// # Returns
    /// - `Some(&Alliance)` - Alliance data if it was requested and fetched
    /// - `None` - Alliance was not requested in the builder
    pub fn get_alliance(&self, alliance_id: i64) -> Option<&Alliance> {
        self.alliances_map.get(&alliance_id)
    }

    /// Stores all fetched entities to the database within the provided transaction.
    ///
    /// Entities are stored in dependency order to maintain referential integrity:
    /// 1. Factions (handled by fetch state)
    /// 2. Alliances (references factions)
    /// 3. Corporations (references alliances and factions)
    /// 4. Characters (references corporations and factions)
    ///
    /// Each storage method handles its own empty-check logic, so the orchestrator
    /// simply delegates to each method unconditionally. This method consumes the
    /// orchestrator to prevent accidental reuse.
    ///
    /// # Arguments
    ///
    /// - `txn` - Database transaction to use for all storage operations
    ///
    /// # Returns
    ///
    /// [`StoredEntities`] containing the database models of all stored entities,
    /// useful for accessing generated IDs and timestamps.
    ///
    /// # Errors
    ///
    /// Returns an error if any database operation fails. The transaction should
    /// be rolled back by the caller.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use bifrost::server::{service::eve::orchestrator::EveEntityOrchestrator, error::Error};
    /// # use sea_orm::{DatabaseConnection, TransactionTrait};
    /// # async fn example(db: &DatabaseConnection, esi: &eve_esi::Client) -> Result<(), Error> {
    /// let txn = db.begin().await?;
    ///
    /// let orchestrator = EveEntityOrchestrator::builder(db, esi)
    ///     .character(123456789)
    ///     .build()
    ///     .await?;
    ///
    /// let stored = orchestrator.store(&txn).await?;
    ///
    /// if let Some(character) = stored.get_character(&123456789) {
    ///     println!("Stored character with DB ID: {}", character.id);
    /// }
    ///
    /// txn.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn store(mut self, txn: &DatabaseTransaction) -> Result<StoredEntities, Error> {
        let stored_factions_record_map = self.orchestrate_faction_storage(txn).await?;
        let stored_alliances_record_map = self.orchestrate_alliance_storage(txn).await?;
        let stored_corporations_record_map = self.orchestrate_corporation_storage(txn).await?;
        let stored_characters_record_map = self.orchestrate_character_storage(txn).await?;

        Ok(StoredEntities {
            factions_map: stored_factions_record_map,
            alliances_map: stored_alliances_record_map,
            corporations_map: stored_corporations_record_map,
            characters_map: stored_characters_record_map,
            factions_record_id_map: self.factions_record_id_map,
            alliances_record_id_map: self.alliances_record_id_map,
            corporations_record_id_map: self.corporations_record_id_map,
            characters_record_id_map: self.characters_record_id_map,
        })
    }

    /// Orchestrates the complete faction storage workflow.
    ///
    /// Takes ownership of the faction state, stores it, and updates the record ID map.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveFactionModel>)` - Map of stored faction models
    /// - `Err(Error)` - Database operation failed
    async fn orchestrate_faction_storage(
        &mut self,
        txn: &DatabaseTransaction,
    ) -> Result<HashMap<i64, EveFactionModel>, Error> {
        let factions_state = std::mem::replace(&mut self.factions, FactionFetchState::NotRequested);
        let (stored_factions_record_map, factions_record_id_map) =
            self.store_factions(txn, factions_state).await?;
        self.factions_record_id_map.extend(factions_record_id_map);

        Ok(stored_factions_record_map)
    }

    /// Orchestrates the complete alliance storage workflow.
    ///
    /// Takes ownership of the alliances map, stores them, and updates the record ID map.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveAllianceModel>)` - Map of stored alliance models
    /// - `Err(Error)` - Database operation failed
    async fn orchestrate_alliance_storage(
        &mut self,
        txn: &DatabaseTransaction,
    ) -> Result<HashMap<i64, EveAllianceModel>, Error> {
        let alliances_map = std::mem::take(&mut self.alliances_map);
        let (stored_alliances_record_map, alliances_record_id_map) =
            self.store_alliances(txn, alliances_map).await?;
        self.alliances_record_id_map.extend(alliances_record_id_map);

        Ok(stored_alliances_record_map)
    }

    /// Orchestrates the complete corporation storage workflow.
    ///
    /// Takes ownership of the corporations map, stores them, and updates the record ID map.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveCorporationModel>)` - Map of stored corporation models
    /// - `Err(Error)` - Database operation failed
    async fn orchestrate_corporation_storage(
        &mut self,
        txn: &DatabaseTransaction,
    ) -> Result<HashMap<i64, EveCorporationModel>, Error> {
        let corporations_map = std::mem::take(&mut self.corporations_map);
        let (stored_corporations_record_map, corporations_record_id_map) =
            self.store_corporations(txn, corporations_map).await?;
        self.corporations_record_id_map
            .extend(corporations_record_id_map);

        Ok(stored_corporations_record_map)
    }

    /// Orchestrates the complete character storage workflow.
    ///
    /// Takes ownership of the characters map, stores them, and updates the record ID map.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveCharacterModel>)` - Map of stored character models
    /// - `Err(Error)` - Database operation failed
    async fn orchestrate_character_storage(
        &mut self,
        txn: &DatabaseTransaction,
    ) -> Result<HashMap<i64, EveCharacterModel>, Error> {
        let characters_map = std::mem::take(&mut self.characters_map);
        let (stored_characters_record_map, characters_record_id_map) =
            self.store_characters(txn, characters_map).await?;
        self.characters_record_id_map
            .extend(characters_record_id_map);

        Ok(stored_characters_record_map)
    }

    /// Handles faction storage based on fetch state.
    ///
    /// Encapsulates all faction storage logic, processing entities according to their fetch state:
    /// - `Fresh`: Upserts new faction data from ESI
    /// - `NotModified`: Updates timestamps for revalidated data (304 Not Modified)
    /// - `UpToDate`: Loads existing factions from database
    /// - `NotRequested`: Returns empty maps without database operations
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `factions_state` - The fetch state determining how to handle factions
    ///
    /// # Returns
    /// - `Ok((entity_map, record_id_map))` - Tuple containing:
    ///   - `HashMap<i64, EveFactionModel>` - Map of EVE faction IDs to stored database models
    ///   - `HashMap<i64, i32>` - Map of EVE faction IDs to database record IDs
    /// - `Err(Error::DbErr)` - Database operation failed
    async fn store_factions(
        &self,
        txn: &DatabaseTransaction,
        factions_state: FactionFetchState,
    ) -> Result<(HashMap<i64, EveFactionModel>, HashMap<i64, i32>), Error> {
        let faction_repo = FactionRepository::new(txn);

        let stored_factions = match factions_state {
            FactionFetchState::Fresh(factions_map) => {
                // Store & return updated faction information fetched from ESI
                faction_repo
                    .upsert_many(
                        factions_map
                            .into_iter()
                            .map(|(_, faction)| faction)
                            .collect(),
                    )
                    .await?
            }
            FactionFetchState::NotModified => {
                // Update timestamps for revalidated data (304 Not Modified)
                faction_repo.update_all_timestamps().await?;
                faction_repo.get_all().await?
            }
            FactionFetchState::UpToDate => {
                // Factions are up-to-date, load existing from DB
                faction_repo.get_all().await?
            }
            FactionFetchState::NotRequested => {
                // No factions needed at all
                return Ok((HashMap::new(), HashMap::new()));
            }
        };

        let record_id_map = stored_factions
            .iter()
            .map(|faction| (faction.faction_id, faction.id))
            .collect();

        let factions_map = stored_factions
            .into_iter()
            .map(|faction| (faction.faction_id, faction))
            .collect();

        Ok((factions_map, record_id_map))
    }

    /// Stores alliance entities to the database with faction relationships.
    ///
    /// Upserts all fetched alliances, linking them to their factions if present.
    /// Returns empty maps immediately if no alliances are provided.
    /// Logs warnings for alliances with faction IDs that couldn't be resolved.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `alliances_map` - Map of alliance IDs to alliance ESI data (may be empty)
    ///
    /// # Returns
    /// - `Ok((entity_map, record_id_map))` - Tuple containing:
    ///   - `HashMap<i64, EveAllianceModel>` - Map of EVE alliance IDs to stored database models
    ///   - `HashMap<i64, i32>` - Map of EVE alliance IDs to database record IDs
    /// - `Err(Error::DbErr)` - Database operation failed
    async fn store_alliances(
        &self,
        txn: &DatabaseTransaction,
        alliances_map: HashMap<i64, Alliance>,
    ) -> Result<(HashMap<i64, EveAllianceModel>, HashMap<i64, i32>), Error> {
        if alliances_map.is_empty() {
            return Ok((HashMap::new(), HashMap::new()));
        }

        let alliance_repo = AllianceRepository::new(txn);

        let alliance_relations = alliances_map.into_iter().map(|(alliance_id, alliance)| {
            let faction_record_id = alliance.faction_id.and_then(|faction_id| {
                match self.factions_record_id_map.get(&faction_id) {
                    Some(id) => Some(*id),
                    None => {
                        tracing::warn!(
                            faction_id = %faction_id,
                            alliance_id = %alliance_id,
                            "Failed to find faction record ID in database; alliance will have no related faction"
                        );
                        None
                    }
                }
            });

            (alliance_id, alliance, faction_record_id)
        }).collect();

        let stored_alliances = alliance_repo.upsert_many(alliance_relations).await?;

        let record_id_map = stored_alliances
            .iter()
            .map(|a| (a.alliance_id, a.id))
            .collect();

        let alliances_map = stored_alliances
            .into_iter()
            .map(|a| (a.alliance_id, a))
            .collect();

        Ok((alliances_map, record_id_map))
    }

    /// Stores corporation entities to the database with alliance and faction relationships.
    ///
    /// Upserts all fetched corporations, linking them to their alliances and factions if present.
    /// Returns empty maps immediately if no corporations are provided.
    /// Logs warnings for corporations with alliance or faction IDs that couldn't be resolved.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `corporations_map` - Map of corporation IDs to corporation ESI data (may be empty)
    ///
    /// # Returns
    /// - `Ok((entity_map, record_id_map))` - Tuple containing:
    ///   - `HashMap<i64, EveCorporationModel>` - Map of EVE corporation IDs to stored database models
    ///   - `HashMap<i64, i32>` - Map of EVE corporation IDs to database record IDs
    /// - `Err(Error::DbErr)` - Database operation failed
    async fn store_corporations(
        &self,
        txn: &DatabaseTransaction,
        corporations_map: HashMap<i64, Corporation>,
    ) -> Result<(HashMap<i64, EveCorporationModel>, HashMap<i64, i32>), Error> {
        if corporations_map.is_empty() {
            return Ok((HashMap::new(), HashMap::new()));
        }

        let corporation_repo = CorporationRepository::new(txn);

        let corporation_relations = corporations_map.into_iter().map(|(corporation_id, corporation)| {
            let faction_record_id = corporation.faction_id.and_then(|faction_id| {
                match self.factions_record_id_map.get(&faction_id) {
                    Some(id) => Some(*id),
                    None => {
                        tracing::warn!(
                            faction_id = %faction_id,
                            corporation_id = %corporation_id,
                            "Failed to find faction record ID in database; corporation will have no related faction"
                        );
                        None
                    }
                }
            });

            let alliance_record_id = corporation.alliance_id.and_then(|alliance_id| {
                match self.alliances_record_id_map.get(&alliance_id) {
                    Some(id) => Some(*id),
                    None => {
                        tracing::warn!(
                            alliance_id = %alliance_id,
                            corporation_id = %corporation_id,
                            "Failed to find alliance record ID in database; corporation will have no related alliance"
                        );
                        None
                    }
                }
            });

            (corporation_id, corporation, alliance_record_id, faction_record_id)
        }).collect();

        let stored_corporations = corporation_repo.upsert_many(corporation_relations).await?;

        let record_id_map = stored_corporations
            .iter()
            .map(|c| (c.corporation_id, c.id))
            .collect();

        let corporations_map = stored_corporations
            .into_iter()
            .map(|c| (c.corporation_id, c))
            .collect();

        Ok((corporations_map, record_id_map))
    }

    /// Stores character entities to the database with corporation and faction relationships.
    ///
    /// Upserts all fetched characters, linking them to their corporations and factions if present.
    /// Returns empty maps immediately if no characters are provided.
    /// Characters without resolvable corporations are skipped with error logs, as corporations
    /// are required for character records.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `characters_map` - Map of character IDs to character ESI data (may be empty)
    ///
    /// # Returns
    /// - `Ok((entity_map, record_id_map))` - Tuple containing:
    ///   - `HashMap<i64, EveCharacterModel>` - Map of EVE character IDs to stored database models
    ///   - `HashMap<i64, i32>` - Map of EVE character IDs to database record IDs
    /// - `Err(Error::DbErr)` - Database operation failed
    async fn store_characters(
        &self,
        txn: &DatabaseTransaction,
        characters_map: HashMap<i64, Character>,
    ) -> Result<(HashMap<i64, EveCharacterModel>, HashMap<i64, i32>), Error> {
        if characters_map.is_empty() {
            return Ok((HashMap::new(), HashMap::new()));
        }

        let character_repo = CharacterRepository::new(txn);

        let character_relations = characters_map.into_iter().filter_map(|(character_id, character)| {
            let faction_record_id = character.faction_id.and_then(|faction_id| {
                match self.factions_record_id_map.get(&faction_id) {
                    Some(id) => Some(*id),
                    None => {
                        tracing::warn!(
                            faction_id = %faction_id,
                            character_id = %character_id,
                            "Failed to find faction record ID in database; character will have no related faction"
                        );
                        None
                    }
                }
            });

            let corporation_record_id =
                match self.corporations_record_id_map.get(&character.corporation_id) {
                    Some(id) => *id,
                    None => {
                        tracing::error!(
                            corporation_id = %character.corporation_id,
                            character_id = %character_id,
                            "Failed to find corporation record ID in database; skipping saving character to database due to missing corporation"
                        );

                        return None; // Skip this character
                    }
                };

            Some((character_id, character, corporation_record_id, faction_record_id))
        }).collect();

        let stored_characters = character_repo.upsert_many(character_relations).await?;

        let record_id_map = stored_characters
            .iter()
            .map(|c| (c.character_id, c.id))
            .collect();

        let characters_map = stored_characters
            .into_iter()
            .map(|c| (c.character_id, c))
            .collect();

        Ok((characters_map, record_id_map))
    }
}
