//! EVE data caching using KvCache pattern
//!
//! This module provides cache implementations for EVE Online entities
//! using the KvCache with CacheFetch trait pattern.

use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::{
        alliance::AllianceRepository, character::CharacterRepository,
        corporation::CorporationRepository, faction::FactionRepository,
    },
    error::Error,
};

use super::{CacheFetch, KvCache};

/// Faction entry ID cache
#[derive(Clone, Debug)]
pub struct DbFactionEntryIdCache(KvCache<i64, i32>);

impl Default for DbFactionEntryIdCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, i32> for DbFactionEntryIdCache {
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let faction_repo = FactionRepository::new(db);
        let entries = faction_repo.get_entry_ids_by_faction_ids(&ids).await?;

        // Convert Vec<(i32, i64)> to Vec<(i64, i32)>
        Ok(entries
            .into_iter()
            .map(|(entry_id, faction_id)| (faction_id, entry_id))
            .collect())
    }

    fn kv_cache(&self) -> &KvCache<i64, i32> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, i32> {
        &mut self.0
    }
}

/// Alliance entry ID cache
#[derive(Clone, Debug)]
pub struct DbAllianceEntryIdCache(KvCache<i64, i32>);

impl Default for DbAllianceEntryIdCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, i32> for DbAllianceEntryIdCache {
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let alliance_repo = AllianceRepository::new(db);
        let entries = alliance_repo.get_entry_ids_by_alliance_ids(&ids).await?;

        // Convert Vec<(i32, i64)> to Vec<(i64, i32)>
        Ok(entries
            .into_iter()
            .map(|(entry_id, alliance_id)| (alliance_id, entry_id))
            .collect())
    }

    fn kv_cache(&self) -> &KvCache<i64, i32> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, i32> {
        &mut self.0
    }
}

/// Corporation entry ID cache
#[derive(Clone, Debug)]
pub struct DbCorporationEntryIdCache(KvCache<i64, i32>);

impl Default for DbCorporationEntryIdCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, i32> for DbCorporationEntryIdCache {
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let corporation_repo = CorporationRepository::new(db);
        let entries = corporation_repo
            .get_entry_ids_by_corporation_ids(&ids)
            .await?;

        // Convert Vec<(i32, i64)> to Vec<(i64, i32)>
        Ok(entries
            .into_iter()
            .map(|(entry_id, corporation_id)| (corporation_id, entry_id))
            .collect())
    }

    fn kv_cache(&self) -> &KvCache<i64, i32> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, i32> {
        &mut self.0
    }
}

/// Character entry ID cache
#[derive(Clone, Debug)]
pub struct DbCharacterEntryIdCache(KvCache<i64, i32>);

impl Default for DbCharacterEntryIdCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, i32> for DbCharacterEntryIdCache {
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let character_repo = CharacterRepository::new(db);
        let entries = character_repo.get_entry_ids_by_character_ids(&ids).await?;

        // Convert Vec<(i32, i64)> to Vec<(i64, i32)>
        Ok(entries
            .into_iter()
            .map(|(entry_id, character_id)| (character_id, entry_id))
            .collect())
    }

    fn kv_cache(&self) -> &KvCache<i64, i32> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, i32> {
        &mut self.0
    }
}
