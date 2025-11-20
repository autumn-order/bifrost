use std::collections::HashMap;

use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::{
        alliance::AllianceRepository, character::CharacterRepository,
        corporation::CorporationRepository, faction::FactionRepository,
    },
    error::Error,
};

use entity::{eve_alliance, eve_faction};

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

impl DbFactionEntryIdCache {
    pub fn new() -> Self {
        Self(None)
    }

    pub async fn get(
        &mut self,
        db: &DatabaseConnection,
        faction_id: i64,
    ) -> Result<Option<i32>, Error> {
        let results = self.get_many(db, vec![faction_id]).await?;

        Ok(results
            .into_iter()
            .find(|(id, _)| *id == faction_id)
            .map(|(_, entry_id)| entry_id))
    }

    pub async fn get_many(
        &mut self,
        db: &DatabaseConnection,
        mut faction_ids: Vec<i64>,
    ) -> Result<Vec<(i64, i32)>, Error> {
        if faction_ids.is_empty() {
            return Ok(Vec::new());
        }

        let requested_ids = faction_ids.clone();

        if let Some(ref cached) = self.0 {
            // Filter faction_ids to only keep those NOT in the cache
            faction_ids.retain(|id| !cached.contains_key(id));

            // If no IDs are missing, return all from cache
            if faction_ids.is_empty() {
                let result = requested_ids
                    .iter()
                    .filter_map(|id| cached.get(id).map(|entry_id| (*id, *entry_id)))
                    .collect();
                return Ok(result);
            }
        }

        // Fetch missing faction entry IDs from database
        let faction_repo = FactionRepository::new(db);
        let fetched_entries = faction_repo
            .get_entry_ids_by_faction_ids(&faction_ids)
            .await?;

        // Convert Vec<(i32, i64)> to HashMap<i64, i32> for cache storage
        let mut fetched_map = HashMap::new();
        for (entry_id, faction_id) in fetched_entries {
            fetched_map.insert(faction_id, entry_id);
        }

        // Update cache by merging fetched entries with existing cache
        if let Some(ref mut cached) = self.0 {
            cached.extend(fetched_map);
        } else {
            self.0 = Some(fetched_map);
        }

        // Return all requested entries (from cache and newly fetched)
        let cache = self.0.as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).map(|entry_id| (*id, *entry_id)))
            .collect();

        Ok(result)
    }
}

impl DbAllianceEntryIdCache {
    pub fn new() -> Self {
        Self(None)
    }

    pub async fn get(
        &mut self,
        db: &DatabaseConnection,
        alliance_id: i64,
    ) -> Result<Option<i32>, Error> {
        let results = self.get_many(db, vec![alliance_id]).await?;

        Ok(results
            .into_iter()
            .find(|(id, _)| *id == alliance_id)
            .map(|(_, entry_id)| entry_id))
    }

    pub async fn get_many(
        &mut self,
        db: &DatabaseConnection,
        mut alliance_ids: Vec<i64>,
    ) -> Result<Vec<(i64, i32)>, Error> {
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
                    .filter_map(|id| cached.get(id).map(|entry_id| (*id, *entry_id)))
                    .collect();
                return Ok(result);
            }
        }

        // Fetch missing alliance entry IDs from database
        let alliance_repo = AllianceRepository::new(db);
        let fetched_entries = alliance_repo
            .get_entry_ids_by_alliance_ids(&alliance_ids)
            .await?;

        // Convert Vec<(i32, i64)> to HashMap<i64, i32> for cache storage
        let mut fetched_map = HashMap::new();
        for (entry_id, alliance_id) in fetched_entries {
            fetched_map.insert(alliance_id, entry_id);
        }

        // Update cache by merging fetched entries with existing cache
        if let Some(ref mut cached) = self.0 {
            cached.extend(fetched_map);
        } else {
            self.0 = Some(fetched_map);
        }

        // Return all requested entries (from cache and newly fetched)
        let cache = self.0.as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).map(|entry_id| (*id, *entry_id)))
            .collect();

        Ok(result)
    }
}

impl DbCorporationEntryIdCache {
    pub fn new() -> Self {
        Self(None)
    }

    pub async fn get(
        &mut self,
        db: &DatabaseConnection,
        corporation_id: i64,
    ) -> Result<Option<i32>, Error> {
        let results = self.get_many(db, vec![corporation_id]).await?;

        Ok(results
            .into_iter()
            .find(|(id, _)| *id == corporation_id)
            .map(|(_, entry_id)| entry_id))
    }

    pub async fn get_many(
        &mut self,
        db: &DatabaseConnection,
        mut corporation_ids: Vec<i64>,
    ) -> Result<Vec<(i64, i32)>, Error> {
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
                    .filter_map(|id| cached.get(id).map(|entry_id| (*id, *entry_id)))
                    .collect();
                return Ok(result);
            }
        }

        // Fetch missing corporation entry IDs from database
        let corporation_repo = CorporationRepository::new(db);
        let fetched_entries = corporation_repo
            .get_entry_ids_by_corporation_ids(&corporation_ids)
            .await?;

        // Convert Vec<(i32, i64)> to HashMap<i64, i32> for cache storage
        let mut fetched_map = HashMap::new();
        for (entry_id, corporation_id) in fetched_entries {
            fetched_map.insert(corporation_id, entry_id);
        }

        // Update cache by merging fetched entries with existing cache
        if let Some(ref mut cached) = self.0 {
            cached.extend(fetched_map);
        } else {
            self.0 = Some(fetched_map);
        }

        // Return all requested entries (from cache and newly fetched)
        let cache = self.0.as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).map(|entry_id| (*id, *entry_id)))
            .collect();

        Ok(result)
    }
}

impl DbCharacterEntryIdCache {
    pub fn new() -> Self {
        Self(None)
    }

    pub async fn get(
        &mut self,
        db: &DatabaseConnection,
        character_id: i64,
    ) -> Result<Option<i32>, Error> {
        let results = self.get_many(db, vec![character_id]).await?;

        Ok(results
            .into_iter()
            .find(|(id, _)| *id == character_id)
            .map(|(_, entry_id)| entry_id))
    }

    pub async fn get_many(
        &mut self,
        db: &DatabaseConnection,
        mut character_ids: Vec<i64>,
    ) -> Result<Vec<(i64, i32)>, Error> {
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
                    .filter_map(|id| cached.get(id).map(|entry_id| (*id, *entry_id)))
                    .collect();
                return Ok(result);
            }
        }

        // Fetch missing character entry IDs from database
        let character_repo = CharacterRepository::new(db);
        let fetched_entries = character_repo
            .get_entry_ids_by_character_ids(&character_ids)
            .await?;

        // Convert Vec<(i32, i64)> to HashMap<i64, i32> for cache storage
        let mut fetched_map = HashMap::new();
        for (entry_id, character_id) in fetched_entries {
            fetched_map.insert(character_id, entry_id);
        }

        // Update cache by merging fetched entries with existing cache
        if let Some(ref mut cached) = self.0 {
            cached.extend(fetched_map);
        } else {
            self.0 = Some(fetched_map);
        }

        // Return all requested entries (from cache and newly fetched)
        let cache = self.0.as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).map(|entry_id| (*id, *entry_id)))
            .collect();

        Ok(result)
    }
}

impl DbFactionModelCache {
    pub fn new() -> Self {
        Self(None)
    }

    pub async fn get(
        &mut self,
        db: &DatabaseConnection,
        faction_id: i64,
    ) -> Result<Option<eve_faction::Model>, Error> {
        let mut results = self.get_many(db, vec![faction_id]).await?;

        Ok(results.pop())
    }

    pub async fn get_many(
        &mut self,
        db: &DatabaseConnection,
        mut faction_ids: Vec<i64>,
    ) -> Result<Vec<eve_faction::Model>, Error> {
        if faction_ids.is_empty() {
            return Ok(Vec::new());
        }

        let requested_ids = faction_ids.clone();

        if let Some(ref cached) = self.0 {
            // Filter faction_ids to only keep those NOT in the cache
            faction_ids.retain(|id| !cached.contains_key(id));

            // If no IDs are missing, return all from cache
            if faction_ids.is_empty() {
                let result = requested_ids
                    .iter()
                    .filter_map(|id| cached.get(id).cloned())
                    .collect();
                return Ok(result);
            }
        }

        // Fetch missing faction models from database
        let faction_repo = FactionRepository::new(db);
        let fetched_factions = faction_repo.get_by_faction_ids(&faction_ids).await?;

        // Convert Vec<Model> to HashMap<i64, Model> for cache storage
        let mut fetched_map = HashMap::new();
        for faction in fetched_factions {
            fetched_map.insert(faction.faction_id, faction);
        }

        // Update cache by merging fetched factions with existing cache
        if let Some(ref mut cached) = self.0 {
            cached.extend(fetched_map);
        } else {
            self.0 = Some(fetched_map);
        }

        // Return all requested factions (from cache and newly fetched)
        let cache = self.0.as_ref().unwrap();
        let result = requested_ids
            .iter()
            .filter_map(|id| cache.get(id).cloned())
            .collect();

        Ok(result)
    }
}

impl DbAllianceModelCache {
    pub fn new() -> Self {
        Self(None)
    }

    pub async fn get(
        &mut self,
        db: &DatabaseConnection,
        alliance_id: i64,
    ) -> Result<Option<eve_alliance::Model>, Error> {
        let mut results = self.get_many(db, vec![alliance_id]).await?;

        Ok(results.pop())
    }

    pub async fn get_many(
        &mut self,
        db: &DatabaseConnection,
        mut alliance_ids: Vec<i64>,
    ) -> Result<Vec<eve_alliance::Model>, Error> {
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

        // Fetch missing alliance models from database
        let alliance_repo = AllianceRepository::new(db);
        let fetched_alliances = alliance_repo.get_by_alliance_ids(&alliance_ids).await?;

        // Convert Vec<Model> to HashMap<i64, Model> for cache storage
        let mut fetched_map = HashMap::new();
        for alliance in fetched_alliances {
            fetched_map.insert(alliance.alliance_id, alliance);
        }

        // Update cache by merging fetched alliances with existing cache
        if let Some(ref mut cached) = self.0 {
            cached.extend(fetched_map);
        } else {
            self.0 = Some(fetched_map);
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
