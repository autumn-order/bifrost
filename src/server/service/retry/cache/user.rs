//! User caching using KvCache pattern
//!
//! This module provides cache implementations for user entities
//! using the KvCache with CacheFetch trait pattern.

use sea_orm::DatabaseConnection;

use crate::server::{data::user::UserRepository, error::Error};

use entity::{bifrost_user, eve_character};

use super::{CacheFetch, KvCache};

/// Cache for user entries by user ID
///
/// This cache is used during retry logic to avoid redundant database queries
/// when fetching user information before committing a transaction.
///
/// Key: user_id (i32)
/// Value: (bifrost_user::Model, Option<eve_character::Model>)
#[derive(Clone, Debug)]
pub struct DbUserCache(KvCache<i32, (bifrost_user::Model, Option<eve_character::Model>)>);

impl Default for DbUserCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i32, (bifrost_user::Model, Option<eve_character::Model>)> for DbUserCache {
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i32>,
    ) -> Result<Vec<(i32, (bifrost_user::Model, Option<eve_character::Model>))>, Error> {
        let repo = UserRepository::new(db);
        let results = repo.get_many(&ids).await?;

        // Convert from Vec<(i32, bifrost_user::Model, Option<eve_character::Model>)>
        // to Vec<(i32, (bifrost_user::Model, Option<eve_character::Model>))>
        Ok(results
            .into_iter()
            .map(|(user_id, user, character)| (user_id, (user, character)))
            .collect())
    }

    fn kv_cache(&self) -> &KvCache<i32, (bifrost_user::Model, Option<eve_character::Model>)> {
        &self.0
    }

    fn kv_cache_mut(
        &mut self,
    ) -> &mut KvCache<i32, (bifrost_user::Model, Option<eve_character::Model>)> {
        &mut self.0
    }
}
