//! Tests for WorkerPool dispatcher management.
//!
//! This module verifies the behavior of dispatcher creation and lifecycle, including
//! correct dispatcher count based on concurrency configuration, dispatcher shutdown
//! behavior, and handling of various concurrency levels from minimal to high.

use bifrost::server::worker::{handler::WorkerJobHandler, pool::WorkerPoolConfig};

use super::*;

/// Tests correct dispatcher count after pool starts.
///
/// Verifies that when a pool with 4 max concurrent jobs is started, it
/// creates exactly 1 dispatcher (4 jobs / 40 per dispatcher = 1).
///
/// Expected: dispatcher_count() returns 1 after start
#[tokio::test]
async fn correct_count_after_start() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests dispatcher count resets to zero after stop.
///
/// Verifies that when a pool is stopped, all dispatchers are shut down
/// and the dispatcher count returns to zero.
///
/// Expected: dispatcher_count() returns 0 after stop
#[tokio::test]
async fn count_resets_to_zero_after_stop() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests single dispatcher configuration.
///
/// Verifies that a pool configured with low concurrency (4 jobs) creates
/// exactly one dispatcher when started.
///
/// Expected: dispatcher_count() returns 1
#[tokio::test]
async fn single_dispatcher_for_low_concurrency() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    pool.start().await.expect("Failed to start pool");
    assert_eq!(pool.dispatcher_count().await, 1, "Should have 1 dispatcher");

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

/// Tests multiple dispatchers with high concurrency.
///
/// Verifies that a pool configured with high concurrency (119 jobs) creates
/// the correct number of dispatchers (3) based on the 40 jobs per dispatcher limit.
///
/// Expected: dispatcher_count() returns 3 for 119 jobs
#[tokio::test]
async fn multiple_dispatchers_for_high_concurrency() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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
