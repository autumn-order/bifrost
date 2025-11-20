//! Provides EVE-related retry cache structs & methods

use chrono::Utc;
use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;

use crate::server::{
    data::eve::faction::FactionRepository, error::Error, util::time::effective_faction_cache_expiry,
};

/// Generic trait for ESI entity caching
#[allow(async_fn_in_trait)]
pub trait EsiEntityCacheable<Entity: Clone> {
    /// Get the internal cache
    fn cache(&self) -> &Option<HashMap<i64, Entity>>;

    /// Get mutable access to the internal cache
    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, Entity>>;

    /// Fetch a single entity from ESI
    async fn fetch_one(esi_client: &eve_esi::Client, id: i64) -> Result<Entity, Error>;

    /// Get the entity type name for error messages
    fn entity_name() -> &'static str;

    /// Generic fetch implementation
    async fn fetch(&mut self, esi_client: &eve_esi::Client, id: i64) -> Result<Entity, Error> {
        let mut entities = self.fetch_multiple(esi_client, vec![id]).await?;

        entities.pop().ok_or_else(|| {
            Error::InternalError(format!(
                "{} {} not found after successful ESI fetch. \
                 This may indicate an ESI API issue or cache corruption.",
                Self::entity_name(),
                id
            ))
        })
    }

    /// Generic fetch_multiple implementation
    async fn fetch_multiple(
        &mut self,
        esi_client: &eve_esi::Client,
        mut ids: Vec<i64>,
    ) -> Result<Vec<Entity>, Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let requested_ids = ids.clone();

        if let Some(ref cached) = self.cache() {
            // Filter ids to only keep those NOT in the cache
            ids.retain(|id| !cached.contains_key(id));

            // If no IDs are missing, return all from cache
            if ids.is_empty() {
                let result = requested_ids
                    .iter()
                    .filter_map(|id| cached.get(id).cloned())
                    .collect();
                return Ok(result);
            }
        }

        // Fetch missing entities from ESI in chunks of up to 10 concurrent requests
        use futures::stream::{FuturesUnordered, StreamExt};

        let mut fetched_entities = HashMap::new();

        for chunk in ids.chunks(10) {
            let mut futures = FuturesUnordered::new();

            for &id in chunk {
                let future = async move {
                    let entity = Self::fetch_one(esi_client, id).await?;
                    Ok::<_, Error>((id, entity))
                };
                futures.push(future);
            }

            while let Some(result) = futures.next().await {
                let (id, entity) = result?;
                fetched_entities.insert(id, entity);
            }
        }

        // Update cache by merging fetched entities with existing cache
        if let Some(ref mut cached) = self.cache_mut() {
            cached.extend(fetched_entities);
        } else {
            *self.cache_mut() = Some(fetched_entities);
        }

        // Return all requested entities (from cache and newly fetched)
        let cache = self.cache().as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).cloned())
            .collect();

        Ok(result)
    }
}

#[derive(Clone, Debug, Default)]
pub struct EsiFactionCache(pub Option<Vec<Faction>>);

#[derive(Clone, Debug, Default)]
pub struct EsiAllianceCache(pub Option<HashMap<i64, Alliance>>);

#[derive(Clone, Debug, Default)]
pub struct EsiCorporationCache(pub Option<HashMap<i64, Corporation>>);

#[derive(Clone, Debug, Default)]
pub struct EsiCharacterCache(pub Option<HashMap<i64, Character>>);

impl EsiFactionCache {
    pub fn new() -> Self {
        Self(None)
    }

    /// Get faction from cache or ESI if not cached & not within ESI cache window
    ///
    /// # Returns
    /// - `Some((Vec<Faction>, bool))`: Factions and a flag indicating whether they came from
    ///   the retry cache (`true`) or were freshly fetched from ESI (`false`)
    /// - `None`: Factions not eligible for update due to still being within cache window
    pub async fn fetch(
        &mut self,
        db: &DatabaseConnection,
        esi_client: &eve_esi::Client,
    ) -> Result<Option<(Vec<Faction>, bool)>, Error> {
        // Try to get factions from cache
        if let Some(ref cached) = self.0 {
            let cached_factions = cached.clone();

            return Ok(Some((cached_factions, true)));
        }

        let faction_repo = FactionRepository::new(db);

        let now = Utc::now();
        let effective_expiry = effective_faction_cache_expiry(now)?;

        // If the latest faction entry was updated at or after the effective expiry, skip updating.
        if let Some(latest_faction_model) = faction_repo.get_latest().await? {
            if latest_faction_model.updated_at >= effective_expiry {
                return Ok(None);
            }
        }

        // First attempt: fetch from ESI and cache the result
        let fetched_factions = esi_client.universe().get_factions().await?;
        self.0 = Some(fetched_factions.clone());
        Ok(Some((fetched_factions, false)))
    }
}

impl EsiEntityCacheable<Alliance> for EsiAllianceCache {
    fn cache(&self) -> &Option<HashMap<i64, Alliance>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, Alliance>> {
        &mut self.0
    }

    async fn fetch_one(esi_client: &eve_esi::Client, id: i64) -> Result<Alliance, Error> {
        Ok(esi_client.alliance().get_alliance_information(id).await?)
    }

    fn entity_name() -> &'static str {
        "Alliance"
    }
}

impl EsiEntityCacheable<Corporation> for EsiCorporationCache {
    fn cache(&self) -> &Option<HashMap<i64, Corporation>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, Corporation>> {
        &mut self.0
    }

    async fn fetch_one(esi_client: &eve_esi::Client, id: i64) -> Result<Corporation, Error> {
        Ok(esi_client
            .corporation()
            .get_corporation_information(id)
            .await?)
    }

    fn entity_name() -> &'static str {
        "Corporation"
    }
}

impl EsiEntityCacheable<Character> for EsiCharacterCache {
    fn cache(&self) -> &Option<HashMap<i64, Character>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, Character>> {
        &mut self.0
    }

    async fn fetch_one(esi_client: &eve_esi::Client, id: i64) -> Result<Character, Error> {
        Ok(esi_client
            .character()
            .get_character_public_information(id)
            .await?)
    }

    fn entity_name() -> &'static str {
        "Character"
    }
}
