//! User character caching using KvCache pattern
//!
//! This module provides cache implementations for user character entities
//! using the KvCache with CacheFetch trait pattern.

use sea_orm::DatabaseConnection;

use crate::server::{data::user::user_character::UserCharacterRepository, error::Error};

use entity::{bifrost_user_character, eve_character};

use super::{CacheFetch, KvCache};

/// Cache for user character entries by EVE character ID
///
/// This cache is used for callback scenarios when we know the character ID
/// from claims but not the user ID (since the user isn't logged in yet).
///
/// Characters may not always be owned by a user, hence why the `user_character`
/// model is an Option.
///
/// Key: character_id (i64)
/// Value: (eve_character::Model, Option<bifrost_user_character::Model>)
#[derive(Clone, Debug)]
pub struct DbUserCharacterOwnershipCache(
    KvCache<i64, (eve_character::Model, Option<bifrost_user_character::Model>)>,
);

impl Default for DbUserCharacterOwnershipCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i64, (eve_character::Model, Option<bifrost_user_character::Model>)>
    for DbUserCharacterOwnershipCache
{
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i64>,
    ) -> Result<
        Vec<(
            i64,
            (eve_character::Model, Option<bifrost_user_character::Model>),
        )>,
        Error,
    > {
        let repo = UserCharacterRepository::new(db);
        let results = repo.get_many_by_character_ids(&ids).await?;

        // Convert from Vec<(i64, eve_character::Model, Option<bifrost_user_character::Model>)>
        // to Vec<(i64, (eve_character::Model, Option<bifrost_user_character::Model>))>
        Ok(results
            .into_iter()
            .map(|(character_id, character, user_character)| {
                (character_id, (character, user_character))
            })
            .collect())
    }

    fn kv_cache(
        &self,
    ) -> &KvCache<i64, (eve_character::Model, Option<bifrost_user_character::Model>)> {
        &self.0
    }

    fn kv_cache_mut(
        &mut self,
    ) -> &mut KvCache<i64, (eve_character::Model, Option<bifrost_user_character::Model>)> {
        &mut self.0
    }
}

/// Cache for user character entries by user ID
///
/// This cache is used when the user is logged in and we need to retrieve
/// all character ownership entries for that user.
///
/// Key: user_id (i32)
/// Value: Vec<bifrost_user_character::Model>
#[derive(Clone, Debug)]
pub struct DbUserCharacterCache(KvCache<i32, Vec<bifrost_user_character::Model>>);

impl Default for DbUserCharacterCache {
    fn default() -> Self {
        Self(KvCache::new())
    }
}

impl CacheFetch<i32, Vec<bifrost_user_character::Model>> for DbUserCharacterCache {
    type Context = DatabaseConnection;

    async fn fetch_missing(
        &self,
        db: &Self::Context,
        ids: Vec<i32>,
    ) -> Result<Vec<(i32, Vec<bifrost_user_character::Model>)>, Error> {
        let repo = UserCharacterRepository::new(db);
        let results = repo.get_many_by_user_ids(&ids).await?;

        Ok(results)
    }

    fn kv_cache(&self) -> &KvCache<i32, Vec<bifrost_user_character::Model>> {
        &self.0
    }

    fn kv_cache_mut(&mut self) -> &mut KvCache<i32, Vec<bifrost_user_character::Model>> {
        &mut self.0
    }
}
