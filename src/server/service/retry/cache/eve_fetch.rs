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

impl EsiAllianceCache {
    pub fn new() -> Self {
        Self(None)
    }

    pub async fn fetch(
        &mut self,
        esi_client: &eve_esi::Client,
        alliance_id: i64,
    ) -> Result<Alliance, Error> {
        let mut alliances = self.fetch_multiple(esi_client, vec![alliance_id]).await?;

        alliances.pop().ok_or_else(|| {
            Error::InternalError(format!(
                "Alliance {} not found after successful ESI fetch. \
                 This may indicate an ESI API issue or cache corruption.",
                alliance_id
            ))
        })
    }

    pub async fn fetch_multiple(
        &mut self,
        esi_client: &eve_esi::Client,
        mut alliance_ids: Vec<i64>,
    ) -> Result<Vec<Alliance>, Error> {
        if alliance_ids.is_empty() {
            return Ok(Vec::new());
        }

        let requested_ids = alliance_ids.clone();

        if let Some(ref cached) = self.0 {
            // Filter alliance_ids to only keep those NOT in the cache
            alliance_ids.retain(|id| !cached.contains_key(id));

            // If no IDs are missing, return all from cache
            if alliance_ids.is_empty() {
                let result = requested_ids
                    .iter()
                    .filter_map(|id| cached.get(id).cloned())
                    .collect();
                return Ok(result);
            }
        }

        // Fetch missing alliances from ESI in chunks of up to 10 concurrent requests
        use futures::stream::{FuturesUnordered, StreamExt};

        let mut fetched_alliances = HashMap::new();

        for chunk in alliance_ids.chunks(10) {
            let mut futures = FuturesUnordered::new();

            for &alliance_id in chunk {
                let future = async move {
                    let alliance = esi_client
                        .alliance()
                        .get_alliance_information(alliance_id)
                        .await?;
                    Ok::<_, Error>((alliance_id, alliance))
                };
                futures.push(future);
            }

            while let Some(result) = futures.next().await {
                let (alliance_id, alliance) = result?;
                fetched_alliances.insert(alliance_id, alliance);
            }
        }

        // Update cache by merging fetched alliances with existing cache
        if let Some(ref mut cached) = self.0 {
            cached.extend(fetched_alliances);
        } else {
            self.0 = Some(fetched_alliances);
        }

        // Return all requested alliances (from cache and newly fetched)
        let cache = self.0.as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).cloned())
            .collect();

        Ok(result)
    }
}

impl EsiCorporationCache {
    pub fn new() -> Self {
        Self(None)
    }

    pub async fn fetch(
        &mut self,
        esi_client: &eve_esi::Client,
        corporation_id: i64,
    ) -> Result<Corporation, Error> {
        let mut corporations = self
            .fetch_multiple(esi_client, vec![corporation_id])
            .await?;

        corporations.pop().ok_or_else(|| {
            Error::InternalError(format!(
                "Corporation {} not found after successful ESI fetch. \
                 This may indicate an ESI API issue or cache corruption.",
                corporation_id
            ))
        })
    }

    pub async fn fetch_multiple(
        &mut self,
        esi_client: &eve_esi::Client,
        mut corporation_ids: Vec<i64>,
    ) -> Result<Vec<Corporation>, Error> {
        if corporation_ids.is_empty() {
            return Ok(Vec::new());
        }

        let requested_ids = corporation_ids.clone();

        if let Some(ref cached) = self.0 {
            // Filter corporation_ids to only keep those NOT in the cache
            corporation_ids.retain(|id| !cached.contains_key(id));

            // If no IDs are missing, return all from cache
            if corporation_ids.is_empty() {
                let result = requested_ids
                    .iter()
                    .filter_map(|id| cached.get(id).cloned())
                    .collect();
                return Ok(result);
            }
        }

        // Fetch missing corporations from ESI in chunks of up to 10 concurrent requests
        use futures::stream::{FuturesUnordered, StreamExt};

        let mut fetched_corporations = HashMap::new();

        for chunk in corporation_ids.chunks(10) {
            let mut futures = FuturesUnordered::new();

            for &corporation_id in chunk {
                let future = async move {
                    let corporation = esi_client
                        .corporation()
                        .get_corporation_information(corporation_id)
                        .await?;
                    Ok::<_, Error>((corporation_id, corporation))
                };
                futures.push(future);
            }

            while let Some(result) = futures.next().await {
                let (corporation_id, corporation) = result?;
                fetched_corporations.insert(corporation_id, corporation);
            }
        }

        // Update cache by merging fetched corporations with existing cache
        if let Some(ref mut cached) = self.0 {
            cached.extend(fetched_corporations);
        } else {
            self.0 = Some(fetched_corporations);
        }

        // Return all requested corporations (from cache and newly fetched)
        let cache = self.0.as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).cloned())
            .collect();

        Ok(result)
    }
}

impl EsiCharacterCache {
    pub fn new() -> Self {
        Self(None)
    }

    pub async fn fetch(
        &mut self,
        esi_client: &eve_esi::Client,
        character_id: i64,
    ) -> Result<Character, Error> {
        let mut characters = self.fetch_multiple(esi_client, vec![character_id]).await?;

        characters.pop().ok_or_else(|| {
            Error::InternalError(format!(
                "Character {} not found after successful ESI fetch. \
                 This may indicate an ESI API issue or cache corruption.",
                character_id
            ))
        })
    }

    pub async fn fetch_multiple(
        &mut self,
        esi_client: &eve_esi::Client,
        mut character_ids: Vec<i64>,
    ) -> Result<Vec<Character>, Error> {
        if character_ids.is_empty() {
            return Ok(Vec::new());
        }

        let requested_ids = character_ids.clone();

        if let Some(ref cached) = self.0 {
            // Filter character_ids to only keep those NOT in the cache
            character_ids.retain(|id| !cached.contains_key(id));

            // If no IDs are missing, return all from cache
            if character_ids.is_empty() {
                let result = requested_ids
                    .iter()
                    .filter_map(|id| cached.get(id).cloned())
                    .collect();
                return Ok(result);
            }
        }

        // Fetch missing characters from ESI in chunks of up to 10 concurrent requests
        use futures::stream::{FuturesUnordered, StreamExt};

        let mut fetched_characters = HashMap::new();

        for chunk in character_ids.chunks(10) {
            let mut futures = FuturesUnordered::new();

            for &character_id in chunk {
                let future = async move {
                    let character = esi_client
                        .character()
                        .get_character_public_information(character_id)
                        .await?;
                    Ok::<_, Error>((character_id, character))
                };
                futures.push(future);
            }

            while let Some(result) = futures.next().await {
                let (character_id, character) = result?;
                fetched_characters.insert(character_id, character);
            }
        }

        // Update cache by merging fetched characters with existing cache
        if let Some(ref mut cached) = self.0 {
            cached.extend(fetched_characters);
        } else {
            self.0 = Some(fetched_characters);
        }

        // Return all requested characters (from cache and newly fetched)
        let cache = self.0.as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).cloned())
            .collect();

        Ok(result)
    }
}
