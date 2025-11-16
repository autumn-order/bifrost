use crate::TestError;
use fred::prelude::*;

/// Redis test setup with automatic cleanup
///
/// This struct manages a Redis connection pool and unique queue name for testing.
/// The queue is automatically cleaned up when the struct is dropped.
pub struct RedisTest {
    pub redis_pool: Pool,
    queue_name: String,
}

impl RedisTest {
    /// Create a new RedisTest instance with a unique queue name
    pub async fn new() -> Result<Self, TestError> {
        let redis_config = Config::from_url("redis://127.0.0.1:6379")?;
        let redis_pool = Pool::new(redis_config, None, None, None, 5)?;
        redis_pool.init().await?;

        let queue_name = Self::generate_unique_queue_name();

        Ok(RedisTest {
            redis_pool,
            queue_name,
        })
    }

    /// Get the unique Redis queue name for this test instance
    ///
    /// This ensures each test uses a unique queue to prevent collisions
    /// when tests run in parallel. The queue name is generated once during
    /// RedisTest creation and cached.
    pub fn queue_name(&self) -> String {
        self.queue_name.clone()
    }

    /// Generate a unique queue name using timestamp and thread ID
    fn generate_unique_queue_name() -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let thread_id = std::thread::current().id();

        let mut hasher = DefaultHasher::new();
        timestamp.hash(&mut hasher);
        thread_id.hash(&mut hasher);
        let hash = hasher.finish();

        format!("test:{}:{:x}:worker:queue", timestamp, hash)
    }
}

impl Drop for RedisTest {
    fn drop(&mut self) {
        // Clean up Redis data when RedisTest is dropped
        // We spawn a task instead of blocking to avoid "runtime within runtime" errors
        let pool = self.redis_pool.clone();
        let queue_name = self.queue_name.clone();

        // Spawn cleanup task in the background
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _: Result<(), fred::error::Error> = pool.del(&queue_name).await;
            });
        }
    }
}
