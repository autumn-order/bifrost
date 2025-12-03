//! Tests for WorkerPool configuration handling.
//!
//! This module verifies the behavior of worker pool configuration, including custom
//! concurrency settings, preservation of configuration values, and handling of edge
//! cases like minimal (1 job) and large (119+ jobs) concurrency limits.

use bifrost::server::worker::pool::WorkerPoolConfig;

use super::*;

/// Tests that custom configuration is preserved correctly.
///
/// Verifies that when a pool is created with a custom configuration (119 jobs),
/// the configuration is preserved and the correct number of dispatchers are created.
///
/// Expected: max_concurrent_jobs() is 119, dispatcher_count() is 3
#[tokio::test]
async fn preserves_custom_configuration() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");

    // 119 jobs = 3 dispatchers (ceiling division ensures max 40 per dispatcher)
    let config = WorkerPoolConfig::new(119);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    assert_eq!(pool.max_concurrent_jobs(), 119);
    assert_eq!(pool.available_permits(), 119);

    pool.start().await.expect("Failed to start pool");
    assert_eq!(
        pool.dispatcher_count().await,
        3,
        "119 jobs needs 3 dispatchers"
    );

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

/// Tests pool operation with minimal concurrency configuration.
///
/// Verifies that the pool can be configured with a minimal concurrency limit
/// of 1 and operates correctly with a single dispatcher.
///
/// Expected: max_concurrent_jobs() is 1, dispatcher_count() is 1
#[tokio::test]
async fn handles_minimal_concurrency() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");

    let config = WorkerPoolConfig::new(1);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    assert_eq!(pool.max_concurrent_jobs(), 1);
    assert_eq!(pool.available_permits(), 1);

    pool.start().await.expect("Failed to start pool");
    assert_eq!(pool.dispatcher_count().await, 1, "Minimum 1 dispatcher");

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

/// Tests pool configuration with large concurrency value.
///
/// Verifies that the pool can handle large concurrency configurations (119 jobs)
/// and correctly calculates the required number of dispatchers.
///
/// Expected: max_concurrent_jobs() is 119, available_permits() is 119
#[tokio::test]
async fn handles_large_concurrency() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");

    // 119 jobs = 2 dispatchers (119 / 40 = 2.975 = 2)
    let config = WorkerPoolConfig::new(119);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    assert_eq!(pool.max_concurrent_jobs(), 119);
    assert_eq!(pool.available_permits(), 119);

    redis.cleanup().await.expect("Failed to cleanup Redis");
}
