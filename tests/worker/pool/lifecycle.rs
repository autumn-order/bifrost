//! Tests for WorkerPool lifecycle management
//!
//! These tests verify the pool's start/stop behavior including:
//! - Starting the pool successfully
//! - Stopping the pool gracefully
//! - Checking if pool is running
//! - Dispatcher count tracking
//! - Idempotent start/stop operations
//! - Pool state transitions

use std::time::Duration;

use bifrost::server::worker::{
    handler::WorkerJobHandler,
    pool::{WorkerPool, WorkerPoolConfig},
};
use bifrost_test_utils::prelude::*;

use crate::util::redis::RedisTest;

use super::setup_test_queue;

/// Create a test-optimized config with fast timeouts for testing
/// Uses 4 max_concurrent_jobs by default for tests (1 dispatcher)
fn test_config() -> WorkerPoolConfig {
    let mut config = WorkerPoolConfig::new(4);
    config.poll_interval_ms = 10;
    config.job_timeout_seconds = 1;
    config.shutdown_timeout_seconds = 1;
    config.cleanup_interval_ms = 100;
    config
}

/// Create a test worker pool with test-optimized config
async fn create_test_pool(test: &TestSetup, redis: &RedisTest) -> WorkerPool {
    let handler = WorkerJobHandler::new(test.db.clone(), test.esi_client.clone());
    let queue = setup_test_queue(redis);

    let config = test_config();
    WorkerPool::new(config, queue, handler)
}

#[tokio::test]
async fn test_pool_starts_successfully() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    let result = pool.start().await;
    assert!(result.is_ok(), "Pool should start successfully");

    assert!(
        pool.is_running().await,
        "Pool should be running after start"
    );

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_stops_successfully() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    pool.start().await.expect("Failed to start pool");
    assert!(pool.is_running().await, "Pool should be running");

    let result = pool.stop().await;
    assert!(result.is_ok(), "Pool should stop successfully");

    assert!(
        !pool.is_running().await,
        "Pool should not be running after stop"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_not_running_initially() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    assert!(
        !pool.is_running().await,
        "Pool should not be running initially"
    );
    assert_eq!(
        pool.dispatcher_count().await,
        0,
        "Dispatcher count should be 0 initially"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_dispatcher_count_after_start() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    pool.start().await.expect("Failed to start pool");

    assert_eq!(
        pool.dispatcher_count().await,
        1,
        "Dispatcher count should be 1 (4 jobs / 40 = 1)"
    );

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_dispatcher_count_after_stop() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    pool.start().await.expect("Failed to start pool");
    assert!(pool.dispatcher_count().await > 0, "Should have dispatchers");

    pool.stop().await.expect("Failed to stop pool");

    assert_eq!(
        pool.dispatcher_count().await,
        0,
        "Dispatcher count should be 0 after stop"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_idempotent_start() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    // Start once
    let result1 = pool.start().await;
    assert!(result1.is_ok(), "First start should succeed");

    let dispatcher_count_1 = pool.dispatcher_count().await;

    // Start again (should be idempotent)
    let result2 = pool.start().await;
    assert!(result2.is_ok(), "Second start should also succeed");

    let dispatcher_count_2 = pool.dispatcher_count().await;

    // Should not create additional dispatchers
    assert_eq!(
        dispatcher_count_1, dispatcher_count_2,
        "Dispatcher count should remain the same"
    );

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_idempotent_stop() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    pool.start().await.expect("Failed to start pool");

    // Stop once
    let result1 = pool.stop().await;
    assert!(result1.is_ok(), "First stop should succeed");

    // Stop again (should be idempotent)
    let result2 = pool.stop().await;
    assert!(result2.is_ok(), "Second stop should also succeed");

    assert!(!pool.is_running().await, "Pool should still not be running");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_can_restart_after_stop() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    // Start, stop, then start again
    pool.start().await.expect("Failed to start pool");
    assert!(pool.is_running().await, "Pool should be running");

    pool.stop().await.expect("Failed to stop pool");
    assert!(!pool.is_running().await, "Pool should be stopped");

    pool.start().await.expect("Failed to restart pool");
    assert!(
        pool.is_running().await,
        "Pool should be running after restart"
    );

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_with_single_dispatcher() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    pool.start().await.expect("Failed to start pool");
    assert_eq!(pool.dispatcher_count().await, 1, "Should have 1 dispatcher");

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_with_many_dispatchers() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let handler = WorkerJobHandler::new(test.db.clone(), test.esi_client.clone());
    let queue = setup_test_queue(&redis);

    // 119 jobs = 3 dispatchers (ceiling division ensures max 40 per dispatcher)
    let config = WorkerPoolConfig::new(119);
    let pool = WorkerPool::new(config, queue, handler);

    pool.start().await.expect("Failed to start pool");
    assert_eq!(
        pool.dispatcher_count().await,
        3,
        "Should have 3 dispatchers (119 jobs needs 3 dispatchers)"
    );

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_cleanup_task_starts_with_pool() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let handler = WorkerJobHandler::new(test.db.clone(), test.esi_client.clone());
    let queue = setup_test_queue(&redis);

    let config = test_config();
    let pool = WorkerPool::new(config, queue.clone(), handler);

    assert!(
        !queue.is_cleanup_running().await,
        "Cleanup should not be running before pool starts"
    );

    pool.start().await.expect("Failed to start pool");

    // Give cleanup task time to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        queue.is_cleanup_running().await,
        "Cleanup should be running after pool starts"
    );

    pool.stop().await.expect("Failed to stop pool");

    // Give cleanup task time to stop
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        !queue.is_cleanup_running().await,
        "Cleanup should not be running after pool stops"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_stop_without_start() {
    let test = TestBuilder::new().build().await.expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    // Stopping without starting should be safe (idempotent)
    let result = pool.stop().await;
    assert!(result.is_ok(), "Stop without start should succeed");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}
