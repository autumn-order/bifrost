use std::collections::HashMap;

use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::{
        alliance::AllianceRepository, character::CharacterRepository,
        corporation::CorporationRepository, faction::FactionRepository,
    },
    error::Error,
};

use entity::{eve_alliance, eve_character, eve_corporation, eve_faction};

/// Generic trait for database entry ID caching
#[allow(async_fn_in_trait)]
pub trait DbEntryIdCacheable {
    /// Get the internal cache
    fn cache(&self) -> &Option<HashMap<i64, i32>>;

    /// Get mutable access to the internal cache
    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, i32>>;

    /// Fetch missing entry IDs from the database
    /// Returns Vec<(entry_id, entity_id)>
    async fn fetch_missing_entry_ids(
        &self,
        db: &DatabaseConnection,
        ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, Error>;

    /// Generic get implementation
    async fn get(&mut self, db: &DatabaseConnection, id: i64) -> Result<Option<i32>, Error> {
        let results = self.get_many(db, vec![id]).await?;

        Ok(results
            .into_iter()
            .find(|(entity_id, _)| *entity_id == id)
            .map(|(_, entry_id)| entry_id))
    }

    /// Generic get_many implementation
    async fn get_many(
        &mut self,
        db: &DatabaseConnection,
        mut ids: Vec<i64>,
    ) -> Result<Vec<(i64, i32)>, Error> {
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
                    .filter_map(|id| cached.get(id).map(|entry_id| (*id, *entry_id)))
                    .collect();
                return Ok(result);
            }
        }

        // Fetch missing entry IDs from database
        let fetched_entries = self.fetch_missing_entry_ids(db, &ids).await?;

        // Convert Vec<(i32, i64)> to HashMap<i64, i32> for cache storage
        let mut fetched_map = HashMap::new();
        for (entry_id, entity_id) in fetched_entries {
            fetched_map.insert(entity_id, entry_id);
        }

        // Update cache by merging fetched entries with existing cache
        if let Some(ref mut cached) = self.cache_mut() {
            cached.extend(fetched_map);
        } else {
            *self.cache_mut() = Some(fetched_map);
        }

        // Return all requested entries (from cache and newly fetched)
        let cache = self.cache().as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).map(|entry_id| (*id, *entry_id)))
            .collect();

        Ok(result)
    }
}

/// Generic trait for database model caching
#[allow(async_fn_in_trait)]
pub trait DbModelCacheable<Model: Clone> {
    /// Get the internal cache
    fn cache(&self) -> &Option<HashMap<i64, Model>>;

    /// Get mutable access to the internal cache
    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, Model>>;

    /// Fetch missing models from the database
    async fn fetch_missing_models(
        &self,
        db: &DatabaseConnection,
        ids: &[i64],
    ) -> Result<Vec<Model>, Error>;

    /// Extract the entity ID from a model
    fn extract_id(model: &Model) -> i64;

    /// Generic get implementation
    async fn get(&mut self, db: &DatabaseConnection, id: i64) -> Result<Option<Model>, Error> {
        let mut results = self.get_many(db, vec![id]).await?;
        Ok(results.pop())
    }

    /// Generic get_many implementation
    async fn get_many(
        &mut self,
        db: &DatabaseConnection,
        mut ids: Vec<i64>,
    ) -> Result<Vec<Model>, Error> {
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

        // Fetch missing models from database
        let fetched_models = self.fetch_missing_models(db, &ids).await?;

        // Convert Vec<Model> to HashMap<i64, Model> for cache storage
        let mut fetched_map = HashMap::new();
        for model in fetched_models {
            let id = Self::extract_id(&model);
            fetched_map.insert(id, model);
        }

        // Update cache by merging fetched models with existing cache
        if let Some(ref mut cached) = self.cache_mut() {
            cached.extend(fetched_map);
        } else {
            *self.cache_mut() = Some(fetched_map);
        }

        // Return all requested models (from cache and newly fetched)
        let cache = self.cache().as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).cloned())
            .collect();

        Ok(result)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DbFactionEntryIdCache(pub Option<HashMap<i64, i32>>);

#[derive(Clone, Debug, Default)]
pub struct DbAllianceEntryIdCache(pub Option<HashMap<i64, i32>>);

#[derive(Clone, Debug, Default)]
pub struct DbCorporationEntryIdCache(pub Option<HashMap<i64, i32>>);

#[derive(Clone, Debug, Default)]
pub struct DbCharacterEntryIdCache(pub Option<HashMap<i64, i32>>);

#[derive(Clone, Debug, Default)]
pub struct DbFactionModelCache(pub Option<HashMap<i64, eve_faction::Model>>);

#[derive(Clone, Debug, Default)]
pub struct DbAllianceModelCache(pub Option<HashMap<i64, eve_alliance::Model>>);

#[derive(Clone, Debug, Default)]
pub struct DbCorporationModelCache(pub Option<HashMap<i64, eve_corporation::Model>>);

#[derive(Clone, Debug, Default)]
pub struct DbCharacterModelCache(pub Option<HashMap<i64, eve_character::Model>>);

impl DbEntryIdCacheable for DbFactionEntryIdCache {
    fn cache(&self) -> &Option<HashMap<i64, i32>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, i32>> {
        &mut self.0
    }

    async fn fetch_missing_entry_ids(
        &self,
        db: &DatabaseConnection,
        ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, Error> {
        let faction_repo = FactionRepository::new(db);
        Ok(faction_repo.get_entry_ids_by_faction_ids(ids).await?)
    }
}

impl DbEntryIdCacheable for DbAllianceEntryIdCache {
    fn cache(&self) -> &Option<HashMap<i64, i32>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, i32>> {
        &mut self.0
    }

    async fn fetch_missing_entry_ids(
        &self,
        db: &DatabaseConnection,
        ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, Error> {
        let alliance_repo = AllianceRepository::new(db);
        Ok(alliance_repo.get_entry_ids_by_alliance_ids(ids).await?)
    }
}

impl DbEntryIdCacheable for DbCorporationEntryIdCache {
    fn cache(&self) -> &Option<HashMap<i64, i32>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, i32>> {
        &mut self.0
    }

    async fn fetch_missing_entry_ids(
        &self,
        db: &DatabaseConnection,
        ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, Error> {
        let corporation_repo = CorporationRepository::new(db);
        Ok(corporation_repo
            .get_entry_ids_by_corporation_ids(ids)
            .await?)
    }
}

impl DbEntryIdCacheable for DbCharacterEntryIdCache {
    fn cache(&self) -> &Option<HashMap<i64, i32>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, i32>> {
        &mut self.0
    }

    async fn fetch_missing_entry_ids(
        &self,
        db: &DatabaseConnection,
        ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, Error> {
        let character_repo = CharacterRepository::new(db);
        Ok(character_repo.get_entry_ids_by_character_ids(ids).await?)
    }
}

impl DbModelCacheable<eve_faction::Model> for DbFactionModelCache {
    fn cache(&self) -> &Option<HashMap<i64, eve_faction::Model>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, eve_faction::Model>> {
        &mut self.0
    }

    async fn fetch_missing_models(
        &self,
        db: &DatabaseConnection,
        ids: &[i64],
    ) -> Result<Vec<eve_faction::Model>, Error> {
        let faction_repo = FactionRepository::new(db);
        Ok(faction_repo.get_by_faction_ids(ids).await?)
    }

    fn extract_id(model: &eve_faction::Model) -> i64 {
        model.faction_id
    }
}

impl DbModelCacheable<eve_alliance::Model> for DbAllianceModelCache {
    fn cache(&self) -> &Option<HashMap<i64, eve_alliance::Model>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, eve_alliance::Model>> {
        &mut self.0
    }

    async fn fetch_missing_models(
        &self,
        db: &DatabaseConnection,
        ids: &[i64],
    ) -> Result<Vec<eve_alliance::Model>, Error> {
        let alliance_repo = AllianceRepository::new(db);
        Ok(alliance_repo.get_by_alliance_ids(ids).await?)
    }

    fn extract_id(model: &eve_alliance::Model) -> i64 {
        model.alliance_id
    }
}

impl DbModelCacheable<eve_corporation::Model> for DbCorporationModelCache {
    fn cache(&self) -> &Option<HashMap<i64, eve_corporation::Model>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, eve_corporation::Model>> {
        &mut self.0
    }

    async fn fetch_missing_models(
        &self,
        db: &DatabaseConnection,
        ids: &[i64],
    ) -> Result<Vec<eve_corporation::Model>, Error> {
        let corporation_repo = CorporationRepository::new(db);
        Ok(corporation_repo.get_by_corporation_ids(ids).await?)
    }

    fn extract_id(model: &eve_corporation::Model) -> i64 {
        model.corporation_id
    }
}

impl DbModelCacheable<eve_character::Model> for DbCharacterModelCache {
    fn cache(&self) -> &Option<HashMap<i64, eve_character::Model>> {
        &self.0
    }

    fn cache_mut(&mut self) -> &mut Option<HashMap<i64, eve_character::Model>> {
        &mut self.0
    }

    async fn fetch_missing_models(
        &self,
        db: &DatabaseConnection,
        ids: &[i64],
    ) -> Result<Vec<eve_character::Model>, Error> {
        let character_repo = CharacterRepository::new(db);
        Ok(character_repo.get_by_character_ids(ids).await?)
    }

    fn extract_id(model: &eve_character::Model) -> i64 {
        model.character_id
    }
}
