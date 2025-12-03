//! Tests for WorkerPool functionality.
//!
//! This module contains tests for worker pool job processing, concurrency control,
//! lifecycle management, and configuration handling.

use bifrost::server::worker::{
    handler::WorkerJobHandler,
    pool::{WorkerPool, WorkerPoolConfig},
    queue::config::WorkerQueueConfig,
    WorkerQueue,
};
use bifrost_test_utils::prelude::*;

use crate::util::redis::RedisTest;

pub fn setup_test_queue(redis: &RedisTest) -> WorkerQueue {
    let config = WorkerQueueConfig {
        queue_name: redis.queue_name(),
        job_ttl: std::time::Duration::from_secs(5),
        cleanup_interval: std::time::Duration::from_millis(50),
    };

    WorkerQueue::with_config(redis.redis_pool.clone(), config)
}

/// Create a test-optimized config with fast timeouts for testing
/// Uses 4 max_concurrent_jobs by default for tests (1 dispatcher)
pub fn test_config() -> WorkerPoolConfig {
    let mut config = WorkerPoolConfig::new(4);
    config.poll_interval_ms = 10;
    config.job_timeout_seconds = 1;
    config.shutdown_timeout_seconds = 1;
    config.cleanup_interval_ms = 100;
    config
}

/// Create a test worker pool with test-optimized config
pub async fn create_test_pool(test: &TestContext, redis: &RedisTest) -> WorkerPool {
    let handler = WorkerJobHandler::new(test.db.clone(), test.esi_client.clone());
    let queue = setup_test_queue(redis);

    let config = test_config();
    WorkerPool::new(config, queue, handler)
}

/// Create a test worker pool with custom config
pub async fn create_test_pool_with_config(
    test: &TestContext,
    redis: &RedisTest,
    config: WorkerPoolConfig,
) -> WorkerPool {
    let handler = WorkerJobHandler::new(test.db.clone(), test.esi_client.clone());
    let queue = setup_test_queue(redis);

    WorkerPool::new(config, queue, handler)
}

mod configuration;
mod job_processing;
mod lifecycle;
mod permits;
