//! Tests for WorkerPool start and stop operations.
//!
//! This module verifies the behavior of pool lifecycle state transitions, including
//! starting and stopping the pool, checking running state, idempotent operations,
//! restart capabilities, and safe handling of edge cases.

use super::*;

/// Tests successful pool startup.
///
/// Verifies that a worker pool can be started successfully and transitions
/// to a running state, ready to process jobs from the queue.
///
/// Expected: start() returns Ok and is_running() returns true
#[tokio::test]
async fn starts_successfully() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests successful pool shutdown.
///
/// Verifies that a running worker pool can be stopped successfully and
/// transitions to a non-running state, gracefully shutting down dispatchers.
///
/// Expected: stop() returns Ok and is_running() returns false
#[tokio::test]
async fn stops_successfully() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests initial pool state is not running.
///
/// Verifies that a newly created pool is not in a running state and has
/// zero dispatchers before start() is called.
///
/// Expected: is_running() returns false, dispatcher_count() returns 0
#[tokio::test]
async fn not_running_initially() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests that start operation is idempotent.
///
/// Verifies that calling start() multiple times on a pool does not create
/// additional dispatchers or cause errors, maintaining consistent state.
///
/// Expected: Multiple starts succeed without changing dispatcher count
#[tokio::test]
async fn start_is_idempotent() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests that stop operation is idempotent.
///
/// Verifies that calling stop() multiple times on a pool does not cause
/// errors or unexpected behavior, safely handling redundant stop calls.
///
/// Expected: Multiple stops succeed without errors
#[tokio::test]
async fn stop_is_idempotent() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests that pool can be restarted after stopping.
///
/// Verifies that a pool can go through multiple start/stop cycles without
/// errors, properly reinitializing dispatchers on each start.
///
/// Expected: Pool can be started, stopped, and started again successfully
#[tokio::test]
async fn can_restart_after_stop() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests stopping pool that was never started.
///
/// Verifies that calling stop() on a pool that hasn't been started is
/// safe and doesn't cause errors, handling the edge case gracefully.
///
/// Expected: stop() succeeds without errors
#[tokio::test]
async fn stop_without_start_is_safe() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    // Stopping without starting should be safe (idempotent)
    let result = pool.stop().await;
    assert!(result.is_ok(), "Stop without start should succeed");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}
