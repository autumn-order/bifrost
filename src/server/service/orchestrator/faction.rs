use std::collections::HashSet;

use chrono::Utc;
use eve_esi::model::universe::Faction;
use sea_orm::{DatabaseConnection, DatabaseTransaction};

use crate::server::{
    data::eve::faction::FactionRepository, error::Error, service::orchestrator::OrchestrationCache,
    util::time::effective_faction_cache_expiry,
};

/// Orchestrator for fetching and persisting EVE factions
pub struct FactionOrchestrator<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> FactionOrchestrator<'a> {
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Retrieve faction entry ID from database that corresponds to provided EVE faction ID
    pub async fn get_faction_entry_id(
        &self,
        faction_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Option<i32>, Error> {
        let ids = self
            .get_many_faction_entry_ids(vec![faction_id], cache)
            .await?;

        Ok(ids.into_iter().next().map(|(_, db_id)| db_id))
    }

    /// Retrieve pairs of EVE faction IDs & DB faction IDs from a list of faction IDs
    pub async fn get_many_faction_entry_ids(
        &self,
        faction_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let missing_ids: Vec<i64> = faction_ids
            .iter()
            .filter(|id| !cache.faction_db_id.contains_key(id))
            .copied()
            .collect();

        if missing_ids.is_empty() {
            return Ok(faction_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .faction_db_id
                        .get(id)
                        .cloned()
                        .map(|db_id| (*id, db_id))
                })
                .collect());
        }

        let faction_repo = FactionRepository::new(self.db);

        let retrieved_ids = faction_repo
            .get_entry_ids_by_faction_ids(&missing_ids)
            .await?;

        for (db_id, faction_id) in retrieved_ids {
            cache.faction_db_id.insert(faction_id, db_id);
        }

        Ok(faction_ids
            .iter()
            .filter_map(|id| {
                cache
                    .faction_db_id
                    .get(id)
                    .cloned()
                    .map(|db_id| (*id, db_id))
            })
            .collect())
    }

    /// Retrieve NPC faction information from ESI
    ///
    /// ESI only supports the fetching of all factions at once, additionally, ESI caches alliance
    /// information for 24 hours which resets at 11:05 UTC. This method will check existing
    /// factions in database first, no fetch attempt will be made if the factions we have stored
    /// are already up to date.
    ///
    /// # Arguments
    /// - `cache`: Unified cache for orchestration that prevents duplicate fetching of factions
    ///   during retry attempts.
    ///
    /// # Returns
    /// - `Some`: If factions currently stored are not within the ESI 24 hour faction cache window
    /// - `None`: If factions are already up to date
    pub async fn fetch_factions(
        &self,
        cache: &mut OrchestrationCache,
    ) -> Result<Option<Vec<Faction>>, Error> {
        if !cache.faction_esi.is_empty() {
            return Ok(Some(cache.faction_esi.values().cloned().collect()));
        }

        // Check cache expiry against database
        let faction_repo = FactionRepository::new(self.db);

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
        let fetched_factions = self.esi_client.universe().get_factions().await?;

        for faction in &fetched_factions {
            cache
                .faction_esi
                .insert(faction.faction_id, faction.clone());
        }

        Ok(Some(fetched_factions))
    }

    /// Persist factions fetched from ESI to database if applicable
    pub async fn persist_factions(
        &self,
        txn: &DatabaseTransaction,
        factions: Vec<Faction>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<entity::eve_faction::Model>, Error> {
        if factions.is_empty() {
            return Ok(Vec::new());
        }

        if cache.factions_persisted {
            return Ok(cache.faction_model.values().cloned().collect());
        }

        // Upsert factions to database
        let faction_repo = FactionRepository::new(txn);
        let persisted_models = faction_repo.upsert_many(factions).await?;

        for model in &persisted_models {
            cache.faction_model.insert(model.faction_id, model.clone());
            cache.faction_db_id.insert(model.faction_id, model.id);
        }

        cache.factions_persisted = true;

        Ok(persisted_models)
    }

    /// Check database for provided faction ids, fetch factions from ESI if any are missing
    pub(super) async fn ensure_factions_exist(
        &self,
        faction_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<(), Error> {
        let existing_ids = self
            .get_many_faction_entry_ids(faction_ids.clone(), cache)
            .await?;

        let existing_faction_ids: HashSet<i64> = existing_ids.iter().map(|(id, _)| *id).collect();

        let missing_ids: Vec<i64> = faction_ids
            .into_iter()
            .filter(|id| !existing_faction_ids.contains(id))
            .collect();

        if missing_ids.is_empty() {
            return Ok(());
        }

        // Fetch the factions if any IDs are missing
        self.fetch_factions(cache).await?;

        Ok(())
    }

    /// Persist any factions currently in the ESI cache
    pub(super) async fn persist_cached_factions(
        &self,
        txn: &DatabaseTransaction,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<entity::eve_faction::Model>, Error> {
        let factions: Vec<Faction> = cache.faction_esi.values().cloned().collect();
        self.persist_factions(txn, factions, cache).await
    }
}
