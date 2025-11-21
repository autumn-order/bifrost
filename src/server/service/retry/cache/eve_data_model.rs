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

use entity::{eve_alliance, eve_character, eve_corporation, eve_faction};

use super::{CacheFetch, KvCache};

/// Faction model cache
#[derive(Clone, Debug)]
pub struct DbFactionModelCache(KvCache<i64, eve_faction::Model>);

impl Default for DbFactionModelCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, eve_faction::Model> for DbFactionModelCache {
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, eve_faction::Model)>, Error> {
        let faction_repo = FactionRepository::new(db);
        let models = faction_repo.get_by_faction_ids(&ids).await?;

        // Convert Vec<Model> to Vec<(id, Model)>
        Ok(models
            .into_iter()
            .map(|model| (model.faction_id, model))
            .collect())
    }

    fn kv_cache(&self) -> &KvCache<i64, eve_faction::Model> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, eve_faction::Model> {
        &mut self.0
    }
}

/// Alliance model cache
#[derive(Clone, Debug)]
pub struct DbAllianceModelCache(KvCache<i64, eve_alliance::Model>);

impl Default for DbAllianceModelCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, eve_alliance::Model> for DbAllianceModelCache {
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, eve_alliance::Model)>, Error> {
        let alliance_repo = AllianceRepository::new(db);
        let models = alliance_repo.get_by_alliance_ids(&ids).await?;

        // Convert Vec<Model> to Vec<(id, Model)>
        Ok(models
            .into_iter()
            .map(|model| (model.alliance_id, model))
            .collect())
    }

    fn kv_cache(&self) -> &KvCache<i64, eve_alliance::Model> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, eve_alliance::Model> {
        &mut self.0
    }
}

/// Corporation model cache
#[derive(Clone, Debug)]
pub struct DbCorporationModelCache(KvCache<i64, eve_corporation::Model>);

impl Default for DbCorporationModelCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, eve_corporation::Model> for DbCorporationModelCache {
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, eve_corporation::Model)>, Error> {
        let corporation_repo = CorporationRepository::new(db);
        let models = corporation_repo.get_by_corporation_ids(&ids).await?;

        // Convert Vec<Model> to Vec<(id, Model)>
        Ok(models
            .into_iter()
            .map(|model| (model.corporation_id, model))
            .collect())
    }

    fn kv_cache(&self) -> &KvCache<i64, eve_corporation::Model> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, eve_corporation::Model> {
        &mut self.0
    }
}

/// Character model cache
#[derive(Clone, Debug)]
pub struct DbCharacterModelCache(KvCache<i64, eve_character::Model>);

impl Default for DbCharacterModelCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, eve_character::Model> for DbCharacterModelCache {
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<Vec<(i64, eve_character::Model)>, Error> {
        let character_repo = CharacterRepository::new(db);
        let models = character_repo.get_by_character_ids(&ids).await?;

        // Convert Vec<Model> to Vec<(id, Model)>
        Ok(models
            .into_iter()
            .map(|model| (model.character_id, model))
            .collect())
    }

    fn kv_cache(&self) -> &KvCache<i64, eve_character::Model> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i64, eve_character::Model> {
        &mut self.0
    }
}
