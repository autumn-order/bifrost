use eve_esi::model::universe::Faction;
use sea_orm::{DatabaseConnection, DatabaseTransaction};

use crate::server::{
    data::eve::faction::FactionRepository,
    error::Error,
    service::retry::cache::{
        eve_data_entry::DbFactionEntryIdCache, eve_data_model::DbFactionModelCache,
        eve_fetch::EsiFactionCache, CacheFetch,
    },
};

/// Cache for faction orchestration
/// Contains caches for faction data
#[derive(Clone, Default, Debug)]
pub struct FactionOrchestrationCache {
    pub faction_esi: EsiFactionCache,
    pub faction_db_ids: DbFactionEntryIdCache,
    pub faction_model: DbFactionModelCache,
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
        // Attempt to update factions if not within 24h cache window
        let Some(fetched_factions) = cache.faction_esi.get_all(self.db, self.esi_client).await?
        else {
            return Ok(None);
        };

        // If we successfully fetched factions or retrieved from cache, reset to false as this is
        // likely a retry attempt, any prior persistence attempt would've been rolled back
        // due to a failed transaction.
        cache.already_persisted = false;

        Ok(Some(fetched_factions))
    }

    /// Persist factions fetched from ESI to database if applicable
    ///
    /// Returns vector of all persisted models
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
            return Ok(cache.faction_model.kv_cache().get_all());
        };

        // Upsert factions to database
        let faction_repo = FactionRepository::new(txn);
        let persisted_models = faction_repo.upsert_many(factions).await?;

        // Update the faction_model cache with the persisted models
        let model_cache = cache.faction_model.kv_cache_mut().inner_mut();
        for model in &persisted_models {
            model_cache.insert(model.faction_id, model.clone());
        }

        // Update the faction_db_ids cache with entry_id mappings
        let entry_id_cache = cache.faction_db_ids.kv_cache_mut().inner_mut();
        for model in &persisted_models {
            entry_id_cache.insert(model.faction_id, model.id);
        }

        Ok(persisted_models)
    }
}
