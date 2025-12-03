//! Tests for WorkerPool cleanup task coordination.
//!
//! This module verifies the behavior of the automatic cleanup task that runs alongside
//! the worker pool, including starting and stopping coordination with the pool lifecycle,
//! and ensuring stale jobs are removed from the queue.

use std::time::Duration;

use bifrost::server::worker::{handler::WorkerJobHandler, pool::WorkerPool};

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
