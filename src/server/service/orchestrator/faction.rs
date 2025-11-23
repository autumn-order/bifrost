use std::collections::{HashMap, HashSet};

use chrono::Utc;
use eve_esi::model::universe::Faction;
use sea_orm::{DatabaseConnection, DatabaseTransaction};

use crate::server::{
    data::eve::faction::FactionRepository, error::Error, util::time::effective_faction_cache_expiry,
};

/// Cache for faction orchestration
/// Contains caches for faction data
#[derive(Clone, Default, Debug)]
pub struct FactionOrchestrationCache {
    pub faction_esi: HashMap<i64, Faction>,
    pub faction_model: HashMap<i64, entity::eve_faction::Model>,
    pub faction_db_id: HashMap<i64, i32>,
    // Faction orchestrator is often utilized in multiple places due to it being
    // depended upon by alliances, corporations, and characters. This prevents
    // redundantly attempting to persist factions multiple times after a successful
    // fetch.
    already_persisted: bool,
}

/// Orchestrator to handle the fetching & persisting of factions from EVE Online's ESI
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
        cache: &mut FactionOrchestrationCache,
    ) -> Result<Option<i32>, Error> {
        let ids = self.get_faction_entry_ids(vec![faction_id], cache).await?;

        Ok(ids.into_iter().next().map(|(_, db_id)| db_id))
    }

    /// Retrieve pairs of EVE faction IDs & DB faction IDs from a list of faction IDs
    pub async fn get_faction_entry_ids(
        &self,
        faction_ids: Vec<i64>,
        cache: &mut FactionOrchestrationCache,
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
    /// # Arguments
    /// - `cache`: Cache for the Faction orchestrator that prevents duplicate fetching of factions
    ///   during retry attempts.
    ///
    /// # Returns
    /// - `Some`: If factions currently stored are not within the ESI 24 hour faction cache window
    /// - `None`: If factions are already up to date
    pub async fn fetch_factions(
        &self,
        cache: &mut FactionOrchestrationCache,
    ) -> Result<Option<Vec<Faction>>, Error> {
        // Reset to false as calling this method again indicates a retry, in which case
        // the persistence would've been rolled back to the transaction not completing
        cache.already_persisted = false;

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

    /// Check database for provided faction ids, fetch factions from ESI if any are missing
    pub async fn ensure_factions_exist(
        &self,
        faction_ids: Vec<i64>,
        cache: &mut FactionOrchestrationCache,
    ) -> Result<(), Error> {
        let existing_ids = self
            .get_faction_entry_ids(faction_ids.clone(), cache)
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

    /// Persist factions fetched from ESI to database if applicable
    pub async fn persist_factions(
        &self,
        txn: &DatabaseTransaction,
        factions: Vec<Faction>,
        cache: &mut FactionOrchestrationCache,
    ) -> Result<Vec<entity::eve_faction::Model>, Error> {
        if factions.is_empty() {
            return Ok(Vec::new());
        }

        if cache.already_persisted {
            return Ok(cache.faction_model.values().cloned().collect());
        };

        // Upsert factions to database
        let faction_repo = FactionRepository::new(txn);
        let persisted_models = faction_repo.upsert_many(factions).await?;

        for model in &persisted_models {
            cache.faction_model.insert(model.faction_id, model.clone());
        }

        cache.already_persisted = true;

        Ok(persisted_models)
    }

    /// Persist any factions currently in the ESI cache
    pub async fn persist_cached_factions(
        &self,
        txn: &DatabaseTransaction,
        cache: &mut FactionOrchestrationCache,
    ) -> Result<Vec<entity::eve_faction::Model>, Error> {
        let factions: Vec<Faction> = cache.faction_esi.values().cloned().collect();
        self.persist_factions(txn, factions, cache).await
    }
}
