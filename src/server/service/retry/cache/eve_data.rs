use std::collections::HashMap;

use sea_orm::DatabaseConnection;

use crate::server::{data::eve::faction::FactionRepository, error::Error};

#[derive(Clone, Debug, Default)]
pub struct DbFactionEntryIdCache(pub Option<HashMap<i64, i32>>);

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
