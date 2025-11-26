pub mod eve_data_entry;
pub mod eve_data_model;
pub mod eve_fetch;
pub mod user;
pub mod user_character;

use std::collections::HashMap;
use std::hash::Hash;

use crate::server::error::Error;

/// Generic key-value cache with internal HashMap storage
#[derive(Clone, Debug, Default)]
pub struct KvCache<K, V> {
    cache: HashMap<K, V>,
}

impl<K, V> KvCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Get a single value from cache, fetching if missing
    pub async fn get<Ctx, E, F, Fut>(
        &mut self,
        ctx: &Ctx,
        key: K,
        fetch_fn: F,
    ) -> Result<Option<V>, E>
    where
        F: FnOnce(&Ctx, Vec<K>) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<(K, V)>, E>>,
    {
        let mut results = self.get_many(ctx, vec![key.clone()], fetch_fn).await?;
        Ok(results.pop().map(|(_, v)| v))
    }

    /// Get multiple values from cache, fetching missing ones
    pub async fn get_many<Ctx, E, F, Fut>(
        &mut self,
        ctx: &Ctx,
        keys: Vec<K>,
        fetch_fn: F,
    ) -> Result<Vec<(K, V)>, E>
    where
        F: FnOnce(&Ctx, Vec<K>) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<(K, V)>, E>>,
    {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let requested_keys = keys.clone();

        // Find missing keys
        let missing_keys: Vec<K> = keys
            .into_iter()
            .filter(|k| !self.cache.contains_key(k))
            .collect();

        // If we have missing keys, fetch them
        if !missing_keys.is_empty() {
            let fetched = fetch_fn(ctx, missing_keys).await?;

            // Update cache with fetched values
            for (k, v) in fetched {
                self.cache.insert(k, v);
            }
        }

        // Return all requested values from cache
        let result = requested_keys
            .iter()
            .filter_map(|k| self.cache.get(k).map(|v| (k.clone(), v.clone())))
            .collect();

        Ok(result)
    }

    /// Get all values stored in cache as a Vec
    pub fn get_all(&self) -> Vec<V> {
        self.cache.values().cloned().collect()
    }

    /// Get the internal cache reference
    pub fn inner(&self) -> &HashMap<K, V> {
        &self.cache
    }

    /// Get mutable reference to internal cache
    pub fn inner_mut(&mut self) -> &mut HashMap<K, V> {
        &mut self.cache
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Check if a key exists in cache
    pub fn contains_key(&self, key: &K) -> bool {
        self.cache.contains_key(key)
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

/// Trait for implementing cache fetch logic
#[allow(async_fn_in_trait)]
pub trait CacheFetch<K, V> {
    /// The context type needed for fetching (e.g., DatabaseConnection, Client)
    type Context;

    /// Fetch missing keys from the data source
    ///
    /// Should return a Vec of (key, value) tuples for the requested keys
    async fn fetch_missing(&self, ctx: &Self::Context, keys: Vec<K>) -> Result<Vec<(K, V)>, Error>;

    /// Get the internal KvCache
    fn kv_cache(&self) -> &KvCache<K, V>;

    /// Get mutable access to the internal KvCache
    fn kv_cache_mut(&mut self) -> &mut KvCache<K, V>;

    /// Get a single value, fetching if not in cache
    async fn get(&mut self, ctx: &Self::Context, key: K) -> Result<Option<V>, Error>
    where
        K: Hash + Eq + Clone,
        V: Clone,
    {
        // Check if key is in cache
        if let Some(value) = self.kv_cache().inner().get(&key).cloned() {
            return Ok(Some(value));
        }

        // Fetch missing key
        let fetched = self.fetch_missing(ctx, vec![key.clone()]).await?;

        // Update cache
        let cache = self.kv_cache_mut().inner_mut();
        for (k, v) in fetched {
            cache.insert(k, v);
        }

        // Return the value
        Ok(self.kv_cache().inner().get(&key).cloned())
    }

    /// Get multiple values, fetching missing ones
    async fn get_many(&mut self, ctx: &Self::Context, keys: Vec<K>) -> Result<Vec<(K, V)>, Error>
    where
        K: Hash + Eq + Clone,
        V: Clone,
    {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let requested_keys = keys.clone();

        // Find missing keys
        let missing_keys: Vec<K> = keys
            .into_iter()
            .filter(|k| !self.kv_cache().inner().contains_key(k))
            .collect();

        // If we have missing keys, fetch them
        if !missing_keys.is_empty() {
            let fetched = self.fetch_missing(ctx, missing_keys).await?;

            // Update cache with fetched values
            let cache = self.kv_cache_mut().inner_mut();
            for (k, v) in fetched {
                cache.insert(k, v);
            }
        }

        // Return all requested values from cache
        let cache = self.kv_cache().inner();
        let result = requested_keys
            .iter()
            .filter_map(|k| cache.get(k).map(|v| (k.clone(), v.clone())))
            .collect();

        Ok(result)
    }
}
