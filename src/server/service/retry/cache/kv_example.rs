//! Example usage of KvCache pattern
//!
//! This module demonstrates how to use the KvCache with CacheFetch trait
//! to create simple, ergonomic cache implementations.

use sea_orm::DatabaseConnection;

use crate::server::{data::eve::alliance::AllianceRepository, error::Error};

use entity::eve_alliance;

use super::{CacheFetch, KvCache};

/// Example: Alliance cache using KvCache
///
/// This is all the boilerplate needed:
/// 1. Define a tuple struct wrapping KvCache<K, V>
/// 2. Implement CacheFetch with the fetch_missing logic
#[derive(Clone, Debug)]
pub struct DbAllianceCache(KvCache<i64, eve_alliance::Model>);

impl CacheFetch<i64, eve_alliance::Model> for DbAllianceCache {
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
