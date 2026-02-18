use std::collections::{HashMap, HashSet};

use chrono::Utc;
use eve_esi::{
    model::{
        alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
    },
    CacheStrategy, CachedResponse,
};
use sea_orm::DatabaseConnection;

use super::EveEntityProvider;
use crate::server::{
    data::eve::{
        alliance::AllianceRepository, corporation::CorporationRepository,
        faction::FactionRepository,
    },
    error::Error,
    service::provider::util::effective_faction_cache_expiry,
};

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
/// # use bifrost::server::{service::provider::EveEntityProviderBuilder, error::Error};
/// # use sea_orm::DatabaseConnection;
/// # async fn example(db: &DatabaseConnection, esi: &eve_esi::Client) -> Result<(), Error> {
/// // Fetch characters and their dependencies (corporations, alliances, factions)
/// let provider = EveEntityProviderBuilder::new(db, esi)
///     .character(123456789)
///     .characters(vec![987654321, 111222333])
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct EveEntityProviderBuilder<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,

    // Explicitly requested IDs - always fetch from ESI
    requested_character_ids: HashSet<i64>,
    requested_corporation_ids: HashSet<i64>,
    requested_alliance_ids: HashSet<i64>,

    // Dependency IDs - check DB first, fetch if missing
    dependency_corporation_ids: HashSet<i64>,
    dependency_alliance_ids: HashSet<i64>,
    dependency_faction_ids: HashSet<i64>,

    // Character data we have already fetched which we just need stored and dependencies resolved
    characters_map: HashMap<i64, Character>,
    // Corporation data we have already fetched which we just need stored and dependencies resolved
    corporations_map: HashMap<i64, Corporation>,
}

impl<'a> EveEntityProviderBuilder<'a> {
    /// Creates a new instance of EveEntityProviderBuilder.
    ///
    /// Constructs a builder for fetching EVE Online entities from ESI with intelligent
    /// dependency resolution.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    /// - `esi_client` - ESI API client reference
    ///
    /// # Returns
    /// - `EveEntityProviderBuilder` - New builder instance with empty request sets
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self {
            db,
            esi_client,
            requested_character_ids: Default::default(),
            requested_corporation_ids: Default::default(),
            requested_alliance_ids: Default::default(),
            dependency_corporation_ids: Default::default(),
            dependency_alliance_ids: Default::default(),
            dependency_faction_ids: Default::default(),
            characters_map: Default::default(),
            corporations_map: Default::default(),
        }
    }

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
        self.dependency_corporation_ids
            .insert(esi_character.corporation_id);

        if let Some(faction_id) = esi_character.faction_id {
            self.dependency_faction_ids.insert(faction_id);
        }

        self.characters_map.insert(character_id, esi_character);

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
        if let Some(alliance_id) = esi_corporation.alliance_id {
            self.dependency_alliance_ids.insert(alliance_id);
        }

        if let Some(faction_id) = esi_corporation.faction_id {
            self.dependency_faction_ids.insert(faction_id);
        }

        self.corporations_map
            .insert(corporation_id, esi_corporation);

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

    /// Builds the provider by fetching all requested entities and their dependencies.
    ///
    /// # Process
    ///
    /// 1. Fetch requested characters from ESI
    /// 2. Check database for dependency corporations, fetch missing ones from ESI
    /// 3. Check database for dependency alliances, fetch missing ones from ESI
    /// 4. Check database for dependency factions, fetch if stale & missing
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
    pub async fn build(mut self) -> Result<EveEntityProvider, Error> {
        let fetched_characters = self.fetch_characters().await?;

        self.characters_map.extend(fetched_characters);

        let (corporations_record_id_map, missing_corporation_ids) =
            self.find_existing_corporations().await?;
        self.requested_corporation_ids
            .extend(missing_corporation_ids);
        let fetched_corporations = self.fetch_corporations().await?;

        self.corporations_map.extend(fetched_corporations);

        let (alliances_record_id_map, missing_alliance_ids) =
            self.find_existing_alliances().await?;
        self.requested_alliance_ids.extend(missing_alliance_ids);
        let alliances_map = self.fetch_alliances().await?;

        let (factions_record_id_map, missing_faction_ids) = self.find_existing_factions().await?;
        let factions_map = if missing_faction_ids.len() > 0 {
            // Attempt to fetch factions if any are missing, should only occur if:
            // - First time fetching any entity related to any a faction
            // - A new faction was added to the game before faction update task was ran
            self.fetch_factions_if_stale().await?
        } else {
            // No factions to store later
            None
        };

        Ok(EveEntityProvider {
            factions_map,
            alliances_map,
            corporations_map: self.corporations_map,
            characters_map: self.characters_map,
            factions_record_id_map,
            alliances_record_id_map,
            corporations_record_id_map,
        })
    }

    /// Fetches requested character IDs from ESI and tracks dependencies.
    ///
    /// For each character fetched, adds their corporation to dependencies and
    /// adds their faction to dependencies if they have one.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, Character>)` - Map of character IDs to character data
    /// - `Err(Error::EsiError)` - ESI request failed
    async fn fetch_characters(&mut self) -> Result<HashMap<i64, Character>, Error> {
        let character_ids: Vec<i64> = self.requested_character_ids.iter().copied().collect();
        let mut characters = Vec::new();

        for character_id in character_ids {
            let character = self
                .esi_client
                .character()
                .get_character_public_information(character_id)
                .send()
                .await?;

            self.dependency_corporation_ids
                .insert(character.corporation_id);
            if let Some(faction_id) = character.faction_id {
                self.dependency_faction_ids.insert(faction_id);
            }

            characters.push((character_id, character.data))
        }

        Ok(characters.into_iter().collect())
    }

    /// Fetches requested corporation IDs from ESI and tracks dependencies.
    ///
    /// For each corporation fetched, adds their alliance and faction to dependencies
    /// if they have them.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, Corporation>)` - Map of corporation IDs to corporation data
    /// - `Err(Error::EsiError)` - ESI request failed
    async fn fetch_corporations(&mut self) -> Result<HashMap<i64, Corporation>, Error> {
        let mut corporations = Vec::new();

        let corporation_ids: Vec<i64> = self.requested_corporation_ids.iter().copied().collect();
        for corporation_id in corporation_ids {
            let corporation = self
                .esi_client
                .corporation()
                .get_corporation_information(corporation_id)
                .send()
                .await?;

            if let Some(alliance_id) = corporation.alliance_id {
                self.dependency_alliance_ids.insert(alliance_id);
            }
            if let Some(faction_id) = corporation.faction_id {
                self.dependency_faction_ids.insert(faction_id);
            }

            corporations.push((corporation_id, corporation.data))
        }

        Ok(corporations.into_iter().collect())
    }

    /// Fetches requested alliance IDs from ESI and tracks dependencies.
    ///
    /// For each alliance fetched, adds their faction to dependencies if they have one.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, Alliance>)` - Map of alliance IDs to alliance data
    /// - `Err(Error::EsiError)` - ESI request failed
    async fn fetch_alliances(&mut self) -> Result<HashMap<i64, Alliance>, Error> {
        let mut alliances = Vec::new();

        let alliance_ids: Vec<i64> = self.requested_alliance_ids.iter().copied().collect();
        for alliance_id in alliance_ids {
            let alliance = self
                .esi_client
                .alliance()
                .get_alliance_information(alliance_id)
                .send()
                .await?;

            if let Some(faction_id) = alliance.faction_id {
                self.dependency_faction_ids.insert(faction_id);
            }

            alliances.push((alliance_id, alliance.data))
        }

        Ok(alliances.into_iter().collect())
    }

    /// Attempts to update factions if last update was not within current cache period.
    ///
    /// Factions are cached for 24 hours expiring daily at 11:05 UTC. Fetches factions if:
    /// - No factions found in the database
    /// - The last updated faction was before the cache expired
    ///
    /// Uses `If-Modified-Since` when existing data is present to minimize data transfer.
    async fn fetch_factions_if_stale(&self) -> Result<Option<HashMap<i64, Faction>>, Error> {
        let faction_repo = FactionRepository::new(self.db);
        let latest_faction = faction_repo.get_latest().await?;

        let fetched_factions = match latest_faction {
            Some(latest) => {
                // Check if has already updated since last cache expiry
                if latest.updated_at < effective_faction_cache_expiry(Utc::now())? {
                    // Faction already up to date, nothing to do
                    return Ok(None);
                }

                // Fetch factions from ESI with If-Modified-Since since we have existing data
                let esi_response = self
                    .esi_client
                    .universe()
                    .get_factions()
                    .send_cached(CacheStrategy::IfModifiedSince(latest.updated_at.and_utc()))
                    .await?;

                let CachedResponse::Fresh(fresh_data) = esi_response else {
                    // Factions have not changed since last request
                    // TODO: update last updated timestamps since all info is currently up to date
                    return Ok(None);
                };

                fresh_data.data
            }
            None => {
                // No existing factions, fetch without If-Modified-Since
                self.esi_client.universe().get_factions().send().await?.data
            }
        };

        Ok(Some(
            fetched_factions
                .into_iter()
                .map(|f| (f.faction_id, f))
                .collect(),
        ))
    }

    /// Finds corporations related to requested entities within the database.
    ///
    /// Queries the database for dependency corporations to avoid redundant ESI calls.
    /// Returns both found corporations and IDs that need to be fetched.
    ///
    /// # Returns
    /// - `Ok((HashMap<i64, i32>, Vec<i64>))` - Tuple of:
    ///   - Map of EVE corporation IDs to their database record IDs
    ///   - Vector of EVE corporation IDs not found in the database
    /// - `Err(Error::DbErr)` - Database query failed
    async fn find_existing_corporations(&self) -> Result<(HashMap<i64, i32>, Vec<i64>), Error> {
        let corporation_repo = CorporationRepository::new(self.db);

        let dependency_corporation_ids: Vec<i64> =
            self.dependency_corporation_ids.iter().copied().collect();
        let corporation_record_ids = corporation_repo
            .get_record_ids_by_corporation_ids(&dependency_corporation_ids)
            .await?;

        let existing_corporation_ids: HashSet<i64> = corporation_record_ids
            .iter()
            .map(|(_, corp_id)| *corp_id)
            .collect();

        let mut missing_corporation_ids = Vec::new();

        for &dep_corp_id in &dependency_corporation_ids {
            if !existing_corporation_ids.contains(&dep_corp_id) {
                missing_corporation_ids.push(dep_corp_id);
            }
        }

        Ok((
            corporation_record_ids
                .into_iter()
                .map(|(record_id, corp_id)| (corp_id, record_id))
                .collect(),
            missing_corporation_ids,
        ))
    }

    /// Finds alliances related to requested entities within the database.
    ///
    /// Queries the database for dependency alliances to avoid redundant ESI calls.
    /// Returns both found alliances and IDs that need to be fetched.
    ///
    /// # Returns
    /// - `Ok((HashMap<i64, i32>, Vec<i64>))` - Tuple of:
    ///   - Map of EVE alliance IDs to their database record IDs
    ///   - Vector of EVE alliance IDs not found in the database
    /// - `Err(Error::DbErr)` - Database query failed
    async fn find_existing_alliances(&self) -> Result<(HashMap<i64, i32>, Vec<i64>), Error> {
        let alliance_repo = AllianceRepository::new(self.db);

        let dependency_alliance_ids: Vec<i64> =
            self.dependency_alliance_ids.iter().copied().collect();
        let alliance_record_ids = alliance_repo
            .get_record_ids_by_alliance_ids(&dependency_alliance_ids)
            .await?;

        let existing_alliance_ids: HashSet<i64> = alliance_record_ids
            .iter()
            .map(|(_, alliance_id)| *alliance_id)
            .collect();

        let mut missing_alliance_ids = Vec::new();

        for &dep_alliance_id in &dependency_alliance_ids {
            if !existing_alliance_ids.contains(&dep_alliance_id) {
                missing_alliance_ids.push(dep_alliance_id);
            }
        }

        Ok((
            alliance_record_ids
                .into_iter()
                .map(|(record_id, alliance_id)| (alliance_id, record_id))
                .collect(),
            missing_alliance_ids,
        ))
    }

    /// Finds factions related to requested entities within the database.
    ///
    /// Queries the database for dependency factions to avoid redundant ESI calls.
    /// Returns both found factions and IDs that need to be fetched.
    ///
    /// # Returns
    /// - `Ok((HashMap<i64, i32>, Vec<i64>))` - Tuple of:
    ///   - Map of EVE faction IDs to their database record IDs
    ///   - Vector of EVE faction IDs not found in the database
    /// - `Err(Error::DbErr)` - Database query failed
    async fn find_existing_factions(&self) -> Result<(HashMap<i64, i32>, Vec<i64>), Error> {
        let faction_repo = FactionRepository::new(self.db);

        let dependency_faction_ids: Vec<i64> =
            self.dependency_faction_ids.iter().copied().collect();
        let faction_record_ids = faction_repo
            .get_record_ids_by_faction_ids(&dependency_faction_ids)
            .await?;

        let existing_faction_ids: HashSet<i64> = faction_record_ids
            .iter()
            .map(|(_, faction_id)| *faction_id)
            .collect();

        let mut missing_faction_ids = Vec::new();

        for &dep_faction_id in &dependency_faction_ids {
            if !existing_faction_ids.contains(&dep_faction_id) {
                missing_faction_ids.push(dep_faction_id);
            }
        }

        Ok((
            faction_record_ids
                .into_iter()
                .map(|(record_id, faction_id)| (faction_id, record_id))
                .collect(),
            missing_faction_ids,
        ))
    }
}
