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
/// use bifrost::server::{
///     service::eve::{orchestrator::EveEntityOrchestrator, esi::EsiProvider},
///     error::AppError
/// };
/// use sea_orm::{DatabaseConnection, TransactionTrait};
///
/// async fn update_character_affiliations(
///     db: &DatabaseConnection,
///     esi_provider: &EsiProvider,
///     character_ids: Vec<i64>,
/// ) -> Result<(), AppError> {
///     let txn = db.begin().await?;
///
///     let orchestrator = EveEntityOrchestrator::builder(db, esi_provider)
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
mod orchestrate;
mod store;
mod stored;
mod util;

use std::collections::HashMap;

use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction};

use crate::server::error::AppError;

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

// ===== Constructor =====
impl EveEntityOrchestrator {
    /// Creates a new builder for fetching and constructing an EVE entity orchestrator.
    ///
    /// This is the primary way to construct an `EveEntityOrchestrator`.
    pub fn builder<'a>(
        db: &'a DatabaseConnection,
        esi_provider: &'a crate::server::service::eve::esi::EsiProvider,
    ) -> EveEntityOrchestratorBuilder<'a> {
        EveEntityOrchestratorBuilder::new(db, esi_provider)
    }
}

// ===== ESI Data Getters =====
impl EveEntityOrchestrator {
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
}

// ===== Storage =====
impl EveEntityOrchestrator {
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
    /// # use bifrost::server::{service::eve::{orchestrator::EveEntityOrchestrator, esi::EsiProvider}, error::AppError};
    /// # use sea_orm::{DatabaseConnection, TransactionTrait};
    /// # async fn example(db: &DatabaseConnection, esi_provider: &EsiProvider) -> Result<(), AppError> {
    /// let txn = db.begin().await?;
    ///
    /// let orchestrator = EveEntityOrchestrator::builder(db, esi_provider)
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
    pub async fn store(mut self, txn: &DatabaseTransaction) -> Result<StoredEntities, AppError> {
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
}
