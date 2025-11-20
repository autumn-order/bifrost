//! Provides EVE-related retry cache structs & methods

use chrono::Utc;
use eve_esi::model::universe::Faction;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::faction::FactionRepository, error::Error, util::time::effective_faction_cache_expiry,
};

#[derive(Clone, Debug, Default)]
pub struct EsiFactionCache(pub Option<Vec<Faction>>);

impl EsiFactionCache {
    pub fn new() -> Self {
        Self(None)
    }

    /// Get faction from cache or ESI if not cached
    ///
    /// # Returns
    /// - `Some((Vec<Faction>, bool))`: Factions and a flag indicating whether they came from
    ///   the retry cache (`true`) or were freshly fetched from ESI (`false`)
    /// - `None`: Factions not eligible for update due to still being within cache window
    pub async fn fetch_factions(
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
