use std::collections::HashMap;

use chrono::Utc;
use eve_esi::{
    model::{alliance::Alliance, character::Character, corporation::Corporation},
    CacheStrategy, CachedResponse,
};
use futures::stream::{self, StreamExt};

use super::{
    super::util::effective_faction_cache_expiry, EveEntityOrchestratorBuilder, FactionFetchState,
};
use crate::server::{data::eve::faction::FactionRepository, error::AppError};

/// Maximum number of concurrent ESI requests to make at once.
/// This balances throughput with respect to ESI rate limits and connection pools.
const MAX_CONCURRENT_ESI_REQUESTS: usize = 20;

impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Fetches character IDs from ESI concurrently.
    ///
    /// Performs concurrent HTTP requests with a limit to respect ESI rate limits.
    /// Stops on first error encountered.
    ///
    /// # Arguments
    /// - `character_ids` - IDs of characters to fetch from ESI
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, Character>)` - Map of character IDs to character data
    /// - `Err(AppError::EsiError)` - ESI request failed
    pub(super) async fn fetch_characters(
        &self,
        character_ids: Vec<i64>,
    ) -> Result<HashMap<i64, Character>, AppError> {
        let characters: Vec<(i64, Character)> = stream::iter(character_ids)
            .map(|character_id| async move {
                let character = self
                    .esi_client
                    .character()
                    .get_character_public_information(character_id)
                    .send()
                    .await?;

                Ok::<_, AppError>((character_id, character.data))
            })
            .buffer_unordered(MAX_CONCURRENT_ESI_REQUESTS)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, AppError>>()?;

        Ok(characters.into_iter().collect())
    }

    /// Fetches corporation IDs from ESI concurrently.
    ///
    /// Performs concurrent HTTP requests with a limit to respect ESI rate limits.
    /// Stops on first error encountered.
    ///
    /// # Arguments
    /// - `corporation_ids` - IDs of corporations to fetch from ESI
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, Corporation>)` - Map of corporation IDs to corporation data
    /// - `Err(AppError::EsiError)` - ESI request failed
    pub(super) async fn fetch_corporations(
        &self,
        corporation_ids: Vec<i64>,
    ) -> Result<HashMap<i64, Corporation>, AppError> {
        let corporations: Vec<(i64, Corporation)> = stream::iter(corporation_ids)
            .map(|corporation_id| async move {
                let corporation = self
                    .esi_client
                    .corporation()
                    .get_corporation_information(corporation_id)
                    .send()
                    .await?;

                Ok::<_, AppError>((corporation_id, corporation.data))
            })
            .buffer_unordered(MAX_CONCURRENT_ESI_REQUESTS)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, AppError>>()?;

        Ok(corporations.into_iter().collect())
    }

    /// Fetches alliance IDs from ESI concurrently.
    ///
    /// Performs concurrent HTTP requests with a limit to respect ESI rate limits.
    /// Stops on first error encountered.
    ///
    /// # Arguments
    /// - `alliance_ids` - IDs of alliances to fetch from ESI
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, Alliance>)` - Map of alliance IDs to alliance data
    /// - `Err(AppError::EsiError)` - ESI request failed
    pub(super) async fn fetch_alliances(
        &self,
        alliance_ids: Vec<i64>,
    ) -> Result<HashMap<i64, Alliance>, AppError> {
        let alliances: Vec<(i64, Alliance)> = stream::iter(alliance_ids)
            .map(|alliance_id| async move {
                let alliance = self
                    .esi_client
                    .alliance()
                    .get_alliance_information(alliance_id)
                    .send()
                    .await?;

                Ok::<_, AppError>((alliance_id, alliance.data))
            })
            .buffer_unordered(MAX_CONCURRENT_ESI_REQUESTS)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, AppError>>()?;

        Ok(alliances.into_iter().collect())
    }

    /// Attempts to update factions if last update was not within current cache period.
    ///
    /// Factions are cached for 24 hours expiring daily at 11:05 UTC. Fetches factions if:
    /// - No factions found in the database
    /// - The last updated faction was before the cache expired
    ///
    /// Uses `If-Modified-Since` when existing data is present to minimize data transfer.
    ///
    /// # Returns
    /// - `Ok(FactionFetchState::Fresh)` - New faction data fetched from ESI
    /// - `Ok(FactionFetchState::NotModified)` - ESI returned 304, data unchanged
    /// - `Ok(FactionFetchState::UpToDate)` - Database factions still within cache period
    /// - `Err(AppError)` - ESI request or database query failed
    pub(super) async fn fetch_factions_if_stale(&self) -> Result<FactionFetchState, AppError> {
        let faction_repo = FactionRepository::new(self.db);
        let latest_faction = faction_repo.get_latest().await?;

        let fetched_factions = match latest_faction {
            Some(latest) => {
                // Check if has already updated since last cache expiry
                if latest.updated_at < effective_faction_cache_expiry(Utc::now())? {
                    // Faction already up to date, nothing to do
                    return Ok(FactionFetchState::UpToDate);
                }

                // Fetch factions from ESI with If-Modified-Since since we have existing data
                let esi_response = self
                    .esi_client
                    .universe()
                    .get_factions()
                    .send_cached(CacheStrategy::IfModifiedSince(latest.updated_at.and_utc()))
                    .await?;

                let CachedResponse::Fresh(fresh_data) = esi_response else {
                    // Factions have not changed since last request (304)
                    // Timestamps will be updated in store() within a transaction
                    return Ok(FactionFetchState::NotModified);
                };

                fresh_data.data
            }
            None => {
                // No existing factions, fetch without If-Modified-Since
                self.esi_client.universe().get_factions().send().await?.data
            }
        };

        Ok(FactionFetchState::Fresh(
            fetched_factions
                .into_iter()
                .map(|f| (f.faction_id, f))
                .collect(),
        ))
    }
}
