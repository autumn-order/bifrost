//! Tests for WorkerPool lifecycle management.
//!
//! This module verifies the behavior of the worker pool's lifecycle operations, including
//! starting and stopping the pool, checking running state, dispatcher count tracking,
//! idempotent operations, state transitions, and cleanup task coordination.

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
async fn create_test_pool(test: &TestContext, redis: &RedisTest) -> WorkerPool {
    let handler = WorkerJobHandler::new(test.db.clone(), test.esi_client.clone());
    let queue = setup_test_queue(redis);

    let config = test_config();
    WorkerPool::new(config, queue, handler)
}

mod start_stop {
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
}

mod dispatcher_management {
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
}

mod cleanup_task {
    use super::*;

    /// Tests that cleanup task starts when pool starts.
    ///
    /// Verifies that the queue cleanup task is automatically started when the
    /// worker pool is started, enabling automatic removal of stale jobs.
    ///
    /// Expected: is_cleanup_running() returns true after pool starts
    #[tokio::test]
    async fn starts_with_pool() {
        let test = TestBuilder::new()
            .build()
            .await
            .expect("Failed to create test setup");
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
}
