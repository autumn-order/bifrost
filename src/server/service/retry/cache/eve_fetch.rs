//! EVE ESI entity caching using KvCache pattern
//!
//! This module provides cache implementations for EVE Online entities
//! fetched from ESI using the KvCache with CacheFetch trait pattern.

use chrono::Utc;
use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};
use futures::stream::{FuturesUnordered, StreamExt};
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::faction::FactionRepository, error::Error, util::time::effective_faction_cache_expiry,
};

use super::{CacheFetch, KvCache};

/// Context for ESI entity fetches (just the ESI client)
pub struct EsiContext<'a> {
    pub client: &'a eve_esi::Client,
}

/// Context for faction fetches (ESI client + database for cache expiry check)
pub struct FactionContext<'a> {
    pub client: &'a eve_esi::Client,
    pub db: &'a DatabaseConnection,
}

/// Faction cache using ESI with database-based cache expiry check
///
/// Unlike other ESI caches, factions are fetched as a complete list rather than
/// by individual IDs, and include a cache expiry check against the database.
#[derive(Clone, Debug)]
pub struct EsiFactionCache(KvCache<i64, Faction>);

impl Default for EsiFactionCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl EsiFactionCache {
    /// Internal helper to fetch factions from ESI with cache expiry check
    ///
    /// Returns `None` if the cache is still valid and doesn't need updating.
    /// Returns `Some(factions)` if factions should be fetched.
    async fn fetch_factions_if_needed(
        ctx: &FactionContext<'_>,
    ) -> Result<Option<Vec<Faction>>, Error> {
        // Check cache expiry against database
        let faction_repo = FactionRepository::new(ctx.db);

        let now = Utc::now();
        let effective_expiry = effective_faction_cache_expiry(now)?;

        // If the latest faction entry was updated at or after the effective expiry, return None
        // to prevent cache updates
        if let Some(latest_faction_model) = faction_repo.get_latest().await? {
            if latest_faction_model.updated_at >= effective_expiry {
                return Ok(None);
            }
        }

        // Fetch all factions from ESI
        let fetched_factions = ctx.client.universe().get_factions().await?;

        Ok(Some(fetched_factions))
    }

    /// Get all factions from cache or fetch from ESI if cache is expired
    ///
    /// Returns `None` if the cache is still valid and doesn't need updating.
    /// Returns `Some(factions)` if factions were fetched and cache was updated.
    ///
    /// This is a convenience wrapper that uses the same logic as `fetch_missing`.
    pub async fn get_all(
        &mut self,
        ctx: &FactionContext<'_>,
    ) -> Result<Option<Vec<Faction>>, Error> {
        // Use the core fetching logic (same as fetch_missing)
        let Some(fetched_factions) = Self::fetch_factions_if_needed(ctx).await? else {
            return Ok(None);
        };

        // Update cache with fetched values
        for faction in &fetched_factions {
            self.0
                .inner_mut()
                .insert(faction.faction_id, faction.clone());
        }

        Ok(Some(fetched_factions))
    }
}

impl CacheFetch<i64, Faction> for EsiFactionCache {
    type Context = FactionContext<'static>;

    async fn fetch_missing(
        &self,
        ctx: &Self::Context,
        _ids: Vec<i64>,
    ) -> Result<Vec<(i64, Faction)>, Error> {
        let Some(fetched_factions) = Self::fetch_factions_if_needed(ctx).await? else {
            // Cache is still valid, return empty vec
            return Ok(Vec::new());
        };

        // Convert to (id, faction) pairs
        Ok(fetched_factions
            .into_iter()
            .map(|faction| (faction.faction_id, faction))
            .collect())
    }

    fn kv_cache(&self) -> &KvCache<i64, Faction> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, Faction> {
        &mut self.0
    }
}

/// Alliance cache using ESI
#[derive(Clone, Debug)]
pub struct EsiAllianceCache(KvCache<i64, Alliance>);

impl Default for EsiAllianceCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, Alliance> for EsiAllianceCache {
    type Context = EsiContext<'static>;

    async fn fetch_missing(
        &self,
        ctx: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, Alliance)>, Error> {
        let mut results = Vec::new();

        // Fetch entities in chunks of 10 concurrent requests
        for chunk in ids.chunks(10) {
            let mut futures = FuturesUnordered::new();

            for &id in chunk {
                let future = async move {
                    let alliance = ctx.client.alliance().get_alliance_information(id).await?;
                    Ok::<_, Error>((id, alliance))
                };
                futures.push(future);
            }

            while let Some(result) = futures.next().await {
                results.push(result?);
            }
        }

        Ok(results)
    }

    fn kv_cache(&self) -> &KvCache<i64, Alliance> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, Alliance> {
        &mut self.0
    }
}

/// Corporation cache using ESI
#[derive(Clone, Debug)]
pub struct EsiCorporationCache(KvCache<i64, Corporation>);

impl Default for EsiCorporationCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, Corporation> for EsiCorporationCache {
    type Context = EsiContext<'static>;

    async fn fetch_missing(
        &self,
        ctx: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, Corporation)>, Error> {
        let mut results = Vec::new();

        // Fetch entities in chunks of 10 concurrent requests
        for chunk in ids.chunks(10) {
            let mut futures = FuturesUnordered::new();

            for &id in chunk {
                let future = async move {
                    let corporation = ctx
                        .client
                        .corporation()
                        .get_corporation_information(id)
                        .await?;
                    Ok::<_, Error>((id, corporation))
                };
                futures.push(future);
            }

            while let Some(result) = futures.next().await {
                results.push(result?);
            }
        }

        Ok(results)
    }

    fn kv_cache(&self) -> &KvCache<i64, Corporation> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, Corporation> {
        &mut self.0
    }
}

/// Character cache using ESI
#[derive(Clone, Debug)]
pub struct EsiCharacterCache(KvCache<i64, Character>);

impl Default for EsiCharacterCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, Character> for EsiCharacterCache {
    type Context = EsiContext<'static>;

    async fn fetch_missing(
        &self,
        ctx: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, Character)>, Error> {
        let mut results = Vec::new();

        // Fetch entities in chunks of 10 concurrent requests
        for chunk in ids.chunks(10) {
            let mut futures = FuturesUnordered::new();

            for &id in chunk {
                let future = async move {
                    let character = ctx
                        .client
                        .character()
                        .get_character_public_information(id)
                        .await?;
                    Ok::<_, Error>((id, character))
                };
                futures.push(future);
            }

            while let Some(result) = futures.next().await {
                results.push(result?);
            }
        }

        Ok(results)
    }

    fn kv_cache(&self) -> &KvCache<i64, Character> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, Character> {
        &mut self.0
    }
}
