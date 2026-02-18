//! # EVE Online Entity Orchestration Provider
//!
//! This module provides a two-phase system for fetching EVE Online entities from ESI
//! and persisting them to the database with proper relationship handling.
//!
//! ## Overview
//!
//! The provider system consists of two main components:
//!
//! - [`EveEntityProviderBuilder`]: Fetches entities from ESI and resolves database relationships
//! - [`EveEntityProvider`]: Stores fetched entities to the database with relationship integrity
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
//! ```no_run
//! use bifrost::server::{service::provider::EveEntityProviderBuilder, error::Error};
//! use sea_orm::{DatabaseConnection, TransactionTrait};
//!
//! async fn update_character_affiliations(
//!     db: &DatabaseConnection,
//!     esi_client: &eve_esi::Client,
//!     character_ids: Vec<i64>,
//! ) -> Result<(), Error> {
//!     let txn = db.begin().await?;
//!
//!     let provider = EveEntityProviderBuilder::new(db, esi_client)
//!         .characters(character_ids)
//!         .build()
//!         .await?;
//!
//!     let stored = provider.store(&txn).await?;
//!     txn.commit().await?;
//!
//!     Ok(())
//! }
//! ```
mod builder;
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
use sea_orm::DatabaseTransaction;

use crate::server::{
    data::eve::{
        alliance::AllianceRepository, character::CharacterRepository,
        corporation::CorporationRepository, faction::FactionRepository,
    },
    error::Error,
};

pub use builder::EveEntityProviderBuilder;

/// Result of fetching factions from ESI.
///
/// This enum prevents illegal states by ensuring factions can only be in one of three valid states:
/// - Fresh data was fetched and needs to be stored
/// - ESI returned 304 Not Modified (timestamp update needed)
/// - No fetch was needed (data not stale yet)
pub(super) enum FactionFetchState {
    /// Fresh faction data was fetched from ESI and should be stored
    Fresh(HashMap<i64, Faction>),
    /// ESI returned 304 Not Modified - only timestamp update needed
    NotModified,
    /// Factions are not stale - no action needed
    NotFetched,
}

/// Provides EVE Entities fetched from ESI via the [`EveEntityProviderBuilder`].
///
/// Contains fetched ESI data and database relationship mappings needed to store
/// entities with proper foreign key references.
///
/// # Usage
///
/// After building, call [`store()`](Self::store) within a database transaction to persist all entities.
/// The provider consumes itself during storage to prevent reuse.
///
/// # Relationship Integrity
///
/// Entities are stored in dependency order:
/// 1. Factions (if any were fetched)
/// 2. Alliances (references factions)
/// 3. Corporations (references alliances and factions)
/// 4. Characters (references corporations and factions)
pub struct EveEntityProvider {
    // ESI data (what was fetched)
    factions: FactionFetchState,
    alliances_map: HashMap<i64, Alliance>,
    corporations_map: HashMap<i64, Corporation>,
    characters_map: HashMap<i64, Character>,

    // Maps EVE ID -> DB Record ID
    // Used for existing related entity DB records found when building provider
    factions_record_id_map: HashMap<i64, i32>,
    alliances_record_id_map: HashMap<i64, i32>,
    corporations_record_id_map: HashMap<i64, i32>,
}

impl EveEntityProvider {
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
    /// Entities are stored in dependency order to maintain referential integrity.
    /// This method consumes the provider to prevent double-storage.
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
    /// # use bifrost::server::{service::provider::EveEntityProviderBuilder, error::Error};
    /// # use sea_orm::{DatabaseConnection, TransactionTrait};
    /// # async fn example(db: &DatabaseConnection, esi: &eve_esi::Client) -> Result<(), Error> {
    /// let txn = db.begin().await?;
    ///
    /// let provider = EveEntityProviderBuilder::new(db, esi)
    ///     .character(123456789)
    ///     .build()
    ///     .await?;
    ///
    /// let stored = provider.store(&txn).await?;
    ///
    /// if let Some(character) = stored.get_character(123456789) {
    ///     println!("Stored character with DB ID: {}", character.id);
    /// }
    ///
    /// txn.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn store(mut self, txn: &DatabaseTransaction) -> Result<StoredEntities, Error> {
        let factions = std::mem::replace(&mut self.factions, FactionFetchState::NotFetched);
        let stored_factions_record_map = match factions {
            FactionFetchState::Fresh(factions_map) => {
                let stored_factions_record_map = self.store_factions(txn, factions_map).await?;
                self.factions_record_id_map.extend(
                    stored_factions_record_map
                        .iter()
                        .map(|(faction_id, faction)| (*faction_id, faction.id.clone())),
                );

                stored_factions_record_map
            }
            FactionFetchState::NotModified => {
                // Factions returned 304 Not Modified - update timestamps to indicate data is still current
                let faction_repo = FactionRepository::new(txn);
                faction_repo.update_all_timestamps().await?;
                Default::default()
            }
            FactionFetchState::NotFetched => {
                // Factions are not stale - no action needed
                Default::default()
            }
        };

        let stored_alliances_record_map = if self.alliances_map.len() > 0 {
            let alliances_map = std::mem::take(&mut self.alliances_map);
            let stored_alliances_record_map = self.store_alliances(txn, alliances_map).await?;
            self.alliances_record_id_map.extend(
                stored_alliances_record_map
                    .iter()
                    .map(|(alliance_id, alliance)| (*alliance_id, alliance.id.clone())),
            );

            stored_alliances_record_map
        } else {
            Default::default()
        };

        let stored_corporations_record_map = if self.corporations_map.len() > 0 {
            let corporations_map = std::mem::take(&mut self.corporations_map);
            let stored_corporations_record_map =
                self.store_corporations(txn, corporations_map).await?;
            self.corporations_record_id_map.extend(
                stored_corporations_record_map
                    .iter()
                    .map(|(corporation_id, corporation)| (*corporation_id, corporation.id.clone())),
            );

            stored_corporations_record_map
        } else {
            Default::default()
        };

        let stored_characters_record_map = if self.characters_map.len() > 0 {
            let characters_map = std::mem::take(&mut self.characters_map);
            self.store_characters(txn, characters_map).await?
        } else {
            Default::default()
        };

        Ok(StoredEntities {
            factions_map: stored_factions_record_map,
            alliances_map: stored_alliances_record_map,
            corporations_map: stored_corporations_record_map,
            characters_map: stored_characters_record_map,
        })
    }

    /// Stores faction entities to the database.
    ///
    /// Upserts all fetched factions, updating existing records or creating new ones.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `factions_map` - Map of faction IDs to faction ESI data
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveFactionModel>)` - Map of faction IDs to stored database models
    /// - `Err(Error::DbErr)` - Database operation failed
    async fn store_factions(
        &self,
        txn: &DatabaseTransaction,
        factions_map: HashMap<i64, Faction>,
    ) -> Result<HashMap<i64, EveFactionModel>, Error> {
        let faction_repo = FactionRepository::new(txn);

        let stored_factions = faction_repo
            .upsert_many(
                factions_map
                    .into_iter()
                    .map(|(_, faction)| faction)
                    .collect(),
            )
            .await?;

        Ok(stored_factions
            .into_iter()
            .map(|f| (f.faction_id, f))
            .collect())
    }

    /// Stores alliance entities to the database with faction relationships.
    ///
    /// Upserts all fetched alliances, linking them to their factions if present.
    /// Logs warnings for alliances with faction IDs that couldn't be resolved.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `alliances_map` - Map of alliance IDs to alliance ESI data
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveAllianceModel>)` - Map of alliance IDs to stored database models
    /// - `Err(Error::DbErr)` - Database operation failed
    async fn store_alliances(
        &self,
        txn: &DatabaseTransaction,
        alliances_map: HashMap<i64, Alliance>,
    ) -> Result<HashMap<i64, EveAllianceModel>, Error> {
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

        Ok(stored_alliances
            .into_iter()
            .map(|a| (a.alliance_id, a))
            .collect())
    }

    /// Stores corporation entities to the database with alliance and faction relationships.
    ///
    /// Upserts all fetched corporations, linking them to their alliances and factions if present.
    /// Logs warnings for corporations with alliance or faction IDs that couldn't be resolved.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `corporations_map` - Map of corporation IDs to corporation ESI data
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveCorporationModel>)` - Map of corporation IDs to stored database models
    /// - `Err(Error::DbErr)` - Database operation failed
    async fn store_corporations(
        &self,
        txn: &DatabaseTransaction,
        corporations_map: HashMap<i64, Corporation>,
    ) -> Result<HashMap<i64, EveCorporationModel>, Error> {
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

        Ok(stored_corporations
            .into_iter()
            .map(|c| (c.corporation_id, c))
            .collect())
    }

    /// Stores character entities to the database with corporation and faction relationships.
    ///
    /// Upserts all fetched characters, linking them to their corporations and factions if present.
    /// Characters without resolvable corporations are skipped with error logs, as corporations
    /// are required for character records.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `characters_map` - Map of character IDs to character ESI data
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveCharacterModel>)` - Map of character IDs to stored database models
    /// - `Err(Error::DbErr)` - Database operation failed
    async fn store_characters(
        &self,
        txn: &DatabaseTransaction,
        characters_map: HashMap<i64, Character>,
    ) -> Result<HashMap<i64, EveCharacterModel>, Error> {
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

        Ok(stored_characters
            .into_iter()
            .map(|c| (c.character_id, c))
            .collect())
    }
}

/// Database models of entities stored by [`EveEntityProvider`].
///
/// Provides access to the persisted database records with their generated IDs
/// and timestamps. Maps EVE Online IDs to database models.
///
/// # Example
///
/// ```no_run
/// # use bifrost::server::service::provider::StoredEntities;
/// # fn example(stored: &StoredEntities, char_id: i64) {
/// if let Some(character) = stored.get_character(char_id) {
///     println!("Character DB ID: {}, Created: {}", character.id, character.created_at);
/// }
/// # }
/// ```
pub struct StoredEntities {
    // Maps EVE ID -> DB Model
    factions_map: HashMap<i64, EveFactionModel>,
    alliances_map: HashMap<i64, EveAllianceModel>,
    corporations_map: HashMap<i64, EveCorporationModel>,
    characters_map: HashMap<i64, EveCharacterModel>,
}

impl StoredEntities {
    /// Gets a character from the stored database models by character ID.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID
    ///
    /// # Returns
    /// - `Some(&EveCharacterModel)` - Character database model if it was stored
    /// - `None` - Character was not stored or was skipped
    pub fn get_character(&self, character_id: i64) -> Option<&EveCharacterModel> {
        self.characters_map.get(&character_id)
    }

    /// Gets a corporation from the stored database models by corporation ID.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID
    ///
    /// # Returns
    /// - `Some(&EveCorporationModel)` - Corporation database model if it was stored
    /// - `None` - Corporation was not stored
    pub fn get_corporation(&self, corporation_id: i64) -> Option<&EveCorporationModel> {
        self.corporations_map.get(&corporation_id)
    }

    /// Gets an alliance from the stored database models by alliance ID.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID
    ///
    /// # Returns
    /// - `Some(&EveAllianceModel)` - Alliance database model if it was stored
    /// - `None` - Alliance was not stored
    pub fn get_alliance(&self, alliance_id: i64) -> Option<&EveAllianceModel> {
        self.alliances_map.get(&alliance_id)
    }

    /// Gets a character from the stored database models by character ID, or returns an error.
    ///
    /// This is a convenience method that wraps [`get_character`](Self::get_character) and
    /// converts `None` into a descriptive `InternalError`.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID
    ///
    /// # Returns
    /// - `Ok(&EveCharacterModel)` - Character database model if it was stored
    /// - `Err(Error::InternalError)` - Character was not found after storing
    pub fn get_character_or_err(&self, character_id: i64) -> Result<&EveCharacterModel, Error> {
        self.get_character(character_id).ok_or_else(|| {
            Error::InternalError(format!(
                "Failed to retrieve information for character {} from database after fetching from ESI & storing.",
                character_id
            ))
        })
    }

    /// Gets a corporation from the stored database models by corporation ID, or returns an error.
    ///
    /// This is a convenience method that wraps [`get_corporation`](Self::get_corporation) and
    /// converts `None` into a descriptive `InternalError`.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID
    ///
    /// # Returns
    /// - `Ok(&EveCorporationModel)` - Corporation database model if it was stored
    /// - `Err(Error::InternalError)` - Corporation was not found after storing
    pub fn get_corporation_or_err(
        &self,
        corporation_id: i64,
    ) -> Result<&EveCorporationModel, Error> {
        self.get_corporation(corporation_id).ok_or_else(|| {
            Error::InternalError(format!(
                "Failed to retrieve information for corporation {} from database after fetching from ESI & storing.",
                corporation_id
            ))
        })
    }

    /// Gets an alliance from the stored database models by alliance ID, or returns an error.
    ///
    /// This is a convenience method that wraps [`get_alliance`](Self::get_alliance) and
    /// converts `None` into a descriptive `InternalError`.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID
    ///
    /// # Returns
    /// - `Ok(&EveAllianceModel)` - Alliance database model if it was stored
    /// - `Err(Error::InternalError)` - Alliance was not found after storing
    pub fn get_alliance_or_err(&self, alliance_id: i64) -> Result<&EveAllianceModel, Error> {
        self.get_alliance(alliance_id).ok_or_else(|| {
            Error::InternalError(format!(
                "Failed to retrieve information for alliance {} from database after fetching from ESI & storing.",
                alliance_id
            ))
        })
    }

    /// Gets all factions from the stored database models.
    ///
    /// Returns all faction models that were stored. Useful for bulk faction operations.
    ///
    /// # Returns
    /// - `Vec<EveFactionModel>` - All stored faction database models, or empty vec if:
    ///   - Factions returned 304 Not Modified (only timestamps were updated)
    ///   - Factions were not stale (no fetch was needed)
    ///   - No factions were requested in the builder
    pub fn get_all_factions(&self) -> Vec<EveFactionModel> {
        self.factions_map.values().cloned().collect()
    }
}
