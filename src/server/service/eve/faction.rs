use chrono::Utc;
use eve_esi::model::universe::Faction;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::faction::FactionRepository,
    error::{eve::EveError, Error},
    service::retry::RetryContext,
    util::time::effective_faction_cache_expiry,
};

#[derive(Clone)]
pub struct FactionService {
    db: DatabaseConnection,
    esi_client: eve_esi::Client,
}

impl FactionService {
    /// Creates a new instance of [`FactionService`]
    pub fn new(db: DatabaseConnection, esi_client: eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Fetches & stores NPC faction information from ESI so long as they aren't within cache period
    ///
    /// The NPC faction cache expires at 11:05 UTC (after downtime)
    pub async fn update_factions(&self) -> Result<Vec<entity::eve_faction::Model>, Error> {
        let mut ctx: RetryContext<Vec<Faction>> = RetryContext::new();
        let faction_service = self.clone();

        ctx.execute_with_retry("faction update", |retry_cache| {
            let faction_service = faction_service.clone();

            Box::pin(async move {
                let faction_repo = FactionRepository::new(faction_service.db.clone());

                let Some((fetched_factions, _)) =
                    faction_service.fetch_factions(retry_cache).await?
                else {
                    return Ok(Vec::new());
                };

                let factions = faction_repo.upsert_many(fetched_factions).await?;

                Ok(factions)
            })
        })
        .await
    }

    /// Attempt to get a faction from the database using its EVE Online faction ID, attempt to update factions if faction is not found
    ///
    /// Faction updates will only occur once per 24 hour period which resets at 11:05 UTC when the EVE ESI
    /// faction cache expires.
    ///
    /// For simply getting a faction without an update, use [`FactionRepository::get_by_faction_id`]
    pub async fn get_or_update_factions(
        &self,
        faction_id: i64,
    ) -> Result<entity::eve_faction::Model, Error> {
        let mut ctx: RetryContext<Vec<Faction>> = RetryContext::new();
        let faction_service = self.clone();

        ctx.execute_with_retry("get or update factions", |retry_cache| {
            let faction_service = faction_service.clone();

            Box::pin(async move {
                let faction_repo = FactionRepository::new(faction_service.db.clone());

                let result = faction_repo.get_by_faction_id(faction_id).await?;

                if let Some(faction) = result {
                    return Ok(faction);
                };

                // If the faction is not found, then a new patch may have come out adding
                // a new faction. Attempt to update factions if they haven't already been
                // updated since downtime.
                let Some((fetched_factions, _)) =
                    faction_service.fetch_factions(retry_cache).await?
                else {
                    // Factions are already up to date - return error
                    return Err(EveError::FactionNotFound(faction_id).into());
                };

                let updated_factions = faction_repo.upsert_many(fetched_factions).await?;

                if let Some(faction) = updated_factions
                    .into_iter()
                    .find(|f| f.faction_id == faction_id)
                {
                    return Ok(faction);
                }

                // Factions have been updated yet still haven't found required faction - return error
                Err(EveError::FactionNotFound(faction_id).into())
            })
        })
        .await
    }

    /// Get faction from cache or ESI if not cached
    ///
    /// # Arguments
    /// - `retry_cache`: Optional Vector containing ESI faction models
    ///
    /// # Returns
    /// - `Some((Vec<Faction>, bool))`: Factions and a flag indicating whether they came from
    ///   the retry cache (`true`) or were freshly fetched from ESI (`false`)
    /// - `None`: Factions not eligible for update due to still being within cache window
    pub(super) async fn fetch_factions(
        &self,
        retry_cache: &mut Option<Vec<Faction>>,
    ) -> Result<Option<(Vec<Faction>, bool)>, Error> {
        // Try to get factions from cache
        if let Some(cached) = retry_cache.as_ref() {
            let cached_factions = cached.clone();

            return Ok(Some((cached_factions, true)));
        }

        let faction_repo = FactionRepository::new(self.db.clone());

        let now = Utc::now();
        let effective_expiry = effective_faction_cache_expiry(now)?;

        // If the latest faction entry was updated at or after the effective expiry, skip updating.
        if let Some(latest_faction_model) = faction_repo.get_latest().await? {
            if latest_faction_model.updated_at >= effective_expiry {
                return Ok(None);
            }
        }

        // First attempt: fetch from ESI and cache the result
        let fetched_factions = self.esi_client.universe().get_factions().await?;
        *retry_cache = Some(fetched_factions.clone());
        Ok(Some((fetched_factions, false)))
    }
}
