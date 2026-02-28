mod fetch;
mod find_existing;
mod orchestrate;

use std::collections::{HashMap, HashSet};

use eve_esi::model::{alliance::Alliance, character::Character, corporation::Corporation};
use sea_orm::DatabaseConnection;

use super::{EveEntityOrchestrator, FactionFetchState};
use crate::server::{error::AppError, service::eve::esi::EsiProvider};

/// Builder for fetching EVE Online entities from ESI with dependency resolution.
///
/// # Strategy
///
/// - **Explicitly requested IDs**: Always fetched fresh from ESI
/// - **Dependency IDs**: Checked in database first, only fetched from ESI if missing
///
/// This minimizes ESI calls while ensuring all required relationships exist in the database.
///
/// # Example
///
/// ```no_run
/// # use bifrost::server::{
/// #    service::eve::{orchestrator::EveEntityOrchestrator, esi::EsiProvider},
/// #    error::AppError,
/// # };
/// # use sea_orm::DatabaseConnection;
/// # async fn example(db: &DatabaseConnection, esi_provider: &EsiProvider) -> Result<(), AppError> {
/// // Fetch characters and their dependencies (corporations, alliances, factions)
/// let orchestrator = EveEntityOrchestrator::builder(db, esi_provider)
///     .character(123456789)
///     .characters(vec![987654321, 111222333])
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct EveEntityOrchestratorBuilder<'a> {
    db: &'a DatabaseConnection,
    esi_provider: &'a EsiProvider,

    // Explicitly request faction fetch (for periodic faction updates)
    requested_faction_update: bool,

    // Explicitly requested IDs - always fetch from ESI
    requested_character_ids: HashSet<i64>,
    requested_corporation_ids: HashSet<i64>,
    requested_alliance_ids: HashSet<i64>,

    // Dependency IDs - check DB first, fetch if missing
    dependency_character_ids: HashSet<i64>,
    dependency_corporation_ids: HashSet<i64>,
    dependency_alliance_ids: HashSet<i64>,
    dependency_faction_ids: HashSet<i64>,

    // ESI data we have already fetched which we just need stored and dependencies resolved
    characters_map: HashMap<i64, Character>,
    corporations_map: HashMap<i64, Corporation>,
    alliances_map: HashMap<i64, Alliance>,
}

// ===== Constructor =====
impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Creates a new instance of EveEntityOrchestratorBuilder.
    ///
    /// Constructs a builder for fetching EVE Online entities from ESI with intelligent
    /// dependency resolution.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    /// - `esi_provider` - ESI provider with circuit breaker protection
    ///
    /// # Returns
    /// - `EveEntityOrchestratorBuilder` - New builder instance with empty request sets
    pub(super) fn new(db: &'a DatabaseConnection, esi_provider: &'a EsiProvider) -> Self {
        Self {
            db,
            esi_provider,
            requested_character_ids: Default::default(),
            requested_corporation_ids: Default::default(),
            requested_alliance_ids: Default::default(),
            dependency_character_ids: Default::default(),
            dependency_corporation_ids: Default::default(),
            dependency_alliance_ids: Default::default(),
            dependency_faction_ids: Default::default(),
            requested_faction_update: false,
            characters_map: Default::default(),
            corporations_map: Default::default(),
            alliances_map: Default::default(),
        }
    }
}

// ===== Character Methods =====
impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Adds a character ID to be fetched from ESI.
    ///
    /// The character will always be fetched fresh from ESI, and its related entities
    /// (corporation, alliance, faction) will be added as dependencies.
    ///
    /// # Arguments
    /// - `id` - EVE Online character ID
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn character(mut self, id: i64) -> Self {
        self.requested_character_ids.insert(id);
        self
    }

    /// Adds multiple character IDs to be fetched from ESI.
    ///
    /// All characters will be fetched fresh from ESI, and their related entities
    /// will be added as dependencies.
    ///
    /// # Arguments
    /// - `ids` - Iterator of EVE Online character IDs
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn characters(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.requested_character_ids.extend(ids);
        self
    }
}

// ===== Corporation Methods =====
impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Adds a corporation ID to be fetched from ESI.
    ///
    /// The corporation will always be fetched fresh from ESI, and its related entities
    /// (alliance, faction) will be added as dependencies.
    ///
    /// # Arguments
    /// - `id` - EVE Online corporation ID
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn corporation(mut self, id: i64) -> Self {
        self.requested_corporation_ids.insert(id);
        self
    }

    /// Adds multiple corporation IDs to be fetched from ESI.
    ///
    /// All corporations will be fetched fresh from ESI, and their related entities
    /// will be added as dependencies.
    ///
    /// # Arguments
    /// - `ids` - Iterator of EVE Online corporation IDs
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn corporations(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.requested_corporation_ids.extend(ids);
        self
    }
}

// ===== Alliance Methods =====
impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Adds an alliance ID to be fetched from ESI.
    ///
    /// The alliance will always be fetched fresh from ESI, and its related faction
    /// (if any) will be added as a dependency.
    ///
    /// # Arguments
    /// - `id` - EVE Online alliance ID
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn alliance(mut self, id: i64) -> Self {
        self.requested_alliance_ids.insert(id);
        self
    }

    /// Adds multiple alliance IDs to be fetched from ESI.
    ///
    /// All alliances will be fetched fresh from ESI, and their related factions
    /// will be added as dependencies.
    ///
    /// # Arguments
    /// - `ids` - Iterator of EVE Online alliance IDs
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn alliances(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.requested_alliance_ids.extend(ids);
        self
    }
}

// ===== Pre-fetched Data Methods =====
impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Adds a character with pre-fetched ESI data.
    ///
    /// This method is used when you already have character data from ESI and want to avoid
    /// fetching it again. The character's related entities (corporation, alliance, faction)
    /// will be added as dependencies to be resolved.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID
    /// - `esi_character` - Pre-fetched character data from ESI
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn character_with_data(mut self, character_id: i64, esi_character: Character) -> Self {
        self.characters_map.insert(character_id, esi_character);
        self
    }

    /// Adds a corporation with pre-fetched ESI data.
    ///
    /// This method is used when you already have corporation data from ESI and want to avoid
    /// fetching it again. The corporation's related entities (alliance, faction)
    /// will be added as dependencies to be resolved.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID
    /// - `esi_corporation` - Pre-fetched corporation data from ESI
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn corporation_with_data(
        mut self,
        corporation_id: i64,
        esi_corporation: Corporation,
    ) -> Self {
        self.corporations_map
            .insert(corporation_id, esi_corporation);
        self
    }

    /// Adds an alliance with pre-fetched ESI data.
    ///
    /// This method is used when you already have alliance data from ESI and want to avoid
    /// fetching it again. The alliance's related entities (faction)
    /// will be added as dependencies to be resolved.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID
    /// - `esi_alliance` - Pre-fetched alliance data from ESI
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn alliance_with_data(mut self, alliance_id: i64, esi_alliance: Alliance) -> Self {
        self.alliances_map.insert(alliance_id, esi_alliance);
        self
    }
}

// ===== Faction Methods =====
impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Explicitly request all factions to be fetched and updated.
    ///
    /// This is used for periodic faction updates where no specific entities are being
    /// fetched, but we want to ensure all faction data is current. The method will:
    /// - Check if factions are stale (based on cache expiry)
    /// - Use If-Modified-Since for efficient updates (304 handling)
    /// - Update timestamps or full data as appropriate
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn with_factions(mut self) -> Self {
        self.requested_faction_update = true;
        self
    }
}

// ===== Dependency Resolution =====
impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Ensures characters exist in the database as dependencies.
    ///
    /// Adds character IDs to the dependency tracking. During build(), the provider will:
    /// - Check which characters already exist in the database
    /// - Only fetch missing characters from ESI
    ///
    /// This is more efficient than `characters()` for bulk operations where most
    /// characters may already exist in the database.
    ///
    /// # Arguments
    /// - `ids` - Character IDs that must exist as dependencies
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn ensure_characters_exist(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.dependency_character_ids.extend(ids);
        self
    }

    /// Ensures corporations exist in the database as dependencies.
    ///
    /// Adds corporation IDs to the dependency tracking. During build(), the provider will:
    /// - Check which corporations already exist in the database
    /// - Only fetch missing corporations from ESI
    ///
    /// This is the recommended method for ensuring corporations exist when processing
    /// bulk affiliation updates or other operations where corporations are dependencies.
    ///
    /// # Arguments
    /// - `ids` - Corporation IDs that must exist as dependencies
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn ensure_corporations_exist(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.dependency_corporation_ids.extend(ids);
        self
    }

    /// Ensures alliances exist in the database as dependencies.
    ///
    /// Adds alliance IDs to the dependency tracking. During build(), the provider will:
    /// - Check which alliances already exist in the database
    /// - Only fetch missing alliances from ESI
    ///
    /// This is the recommended method for ensuring alliances exist when processing
    /// bulk affiliation updates or other operations where alliances are dependencies.
    ///
    /// # Arguments
    /// - `ids` - Alliance IDs that must exist as dependencies
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn ensure_alliances_exist(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.dependency_alliance_ids.extend(ids);
        self
    }

    /// Ensures factions exist in the database as dependencies.
    ///
    /// Adds faction IDs to the dependency tracking. During build(), the provider will:
    /// - Check which factions already exist in the database
    /// - Only fetch missing factions from ESI
    ///
    /// Note: This is different from `with_factions()` which is used for periodic
    /// faction updates. Use this method when you need specific factions to exist
    /// as dependencies for foreign key relationships.
    ///
    /// # Arguments
    /// - `ids` - Faction IDs that must exist as dependencies
    ///
    /// # Returns
    /// - `Self` - Builder instance for method chaining
    pub fn ensure_factions_exist(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.dependency_faction_ids.extend(ids);
        self
    }
}

// ===== Build =====
impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Builds the provider by fetching all requested entities and their dependencies.
    ///
    /// # Process
    ///
    /// 1. Check database for dependency characters, fetch missing ones from ESI
    /// 2. Fetch requested characters from ESI
    /// 3. Check database for dependency corporations, fetch missing ones from ESI
    /// 4. Check database for dependency alliances, fetch missing ones from ESI
    /// 5. Check database for dependency factions, fetch if stale & missing
    ///
    /// # Returns
    ///
    /// An [`EveEntityProvider`] containing all fetched entities and database relationship mappings.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - ESI requests fail
    /// - Database queries fail
    pub async fn build(mut self) -> Result<EveEntityOrchestrator, AppError> {
        let characters_record_id_map = self.orchestrate_characters().await?;
        let corporations_record_id_map = self.orchestrate_corporations().await?;
        let alliances_record_id_map = self.orchestrate_alliances().await?;
        let (factions_record_id_map, factions) = self.orchestrate_factions().await?;

        Ok(EveEntityOrchestrator {
            factions,
            alliances_map: self.alliances_map,
            corporations_map: self.corporations_map,
            characters_map: self.characters_map,
            factions_record_id_map,
            alliances_record_id_map,
            corporations_record_id_map,
            characters_record_id_map,
        })
    }
}
