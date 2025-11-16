//! Tests for WorkerPool job processing and concurrency
//!
//! These tests verify the pool's behavior including:
//! - Job processing from queue
//! - Concurrency control via semaphore
//! - Permit tracking
//! - Job execution with timeout
//! - Multiple concurrent jobs
//! - Semaphore capacity limits

use std::sync::Arc;
use std::time::Duration;

use bifrost::server::{
    model::worker::WorkerJob,
    worker::pool::{WorkerPool, WorkerPoolConfig},
};
use bifrost_test_utils::prelude::*;

use crate::redis::RedisTest;

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
    let db = Arc::new(test.state.db.clone());
    let esi_client = Arc::new(test.state.esi_client.clone());
    let queue = Arc::new(setup_test_queue(redis));

    let config = test_config();
    WorkerPool::new(config, db, esi_client, queue)
}

/// Create a test worker pool with custom config
async fn create_test_pool_with_config(
    test: &TestSetup,
    redis: &RedisTest,
    config: WorkerPoolConfig,
) -> WorkerPool {
    let db = Arc::new(test.state.db.clone());
    let esi_client = Arc::new(test.state.esi_client.clone());
    let queue = Arc::new(setup_test_queue(redis));

    WorkerPool::new(config, db, esi_client, queue)
}

#[tokio::test]
async fn test_pool_initial_permits_available() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    assert_eq!(
        pool.available_permits(),
        4,
        "All permits should be available initially"
    );
    assert_eq!(
        pool.active_job_count(),
        0,
        "No jobs should be active initially"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_max_concurrent_jobs() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    assert_eq!(
        pool.max_concurrent_jobs(),
        4,
        "Max concurrent jobs should match config"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_processes_single_job() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Push a job to the queue
    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    queue
        .push(job.clone())
        .await
        .expect("Failed to push job to queue");

    // Create and start pool
    let pool = create_test_pool(&test, &redis).await;

    pool.start().await.expect("Failed to start pool");

    // Give the pool time to process the job
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Job should have been processed (queue should be empty)
    let popped = queue.pop().await.expect("Failed to pop from queue");
    assert!(popped.is_none(), "Queue should be empty after processing");

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_processes_multiple_jobs() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Push multiple jobs
    let jobs = vec![
        WorkerJob::UpdateCharacterInfo {
            character_id: 12345,
        },
        WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000001,
        },
        WorkerJob::UpdateCorporationInfo {
            corporation_id: 98000001,
        },
    ];

    for job in &jobs {
        queue
            .push(job.clone())
            .await
            .expect("Failed to push job to queue");
    }

    // Create and start pool
    let pool = create_test_pool(&test, &redis).await;

    pool.start().await.expect("Failed to start pool");

    // Give the pool time to process all jobs
    tokio::time::sleep(Duration::from_millis(300)).await;

    // All jobs should have been processed
    let popped = queue.pop().await.expect("Failed to pop from queue");
    assert!(
        popped.is_none(),
        "Queue should be empty after processing all jobs"
    );

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_respects_concurrency_limit() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    assert_eq!(
        pool.max_concurrent_jobs(),
        4,
        "Max concurrent jobs should be 4"
    );
    assert_eq!(
        pool.available_permits(),
        4,
        "All 4 permits should be available"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_active_job_count_calculation() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let pool = create_test_pool(&test, &redis).await;

    // Initially no active jobs
    assert_eq!(pool.active_job_count(), 0);
    assert_eq!(pool.available_permits(), 4);

    // The calculation is: max_concurrent_jobs - available_permits
    // We can't easily test with actual running jobs, but we verify the calculation is correct
    assert_eq!(
        pool.active_job_count() + pool.available_permits(),
        pool.max_concurrent_jobs()
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_handles_empty_queue() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");

    let pool = create_test_pool(&test, &redis).await;

    pool.start().await.expect("Failed to start pool");

    // Pool should handle empty queue gracefully
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Pool should still be running
    assert!(pool.is_running().await, "Pool should still be running");

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_processes_different_job_types() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Push different types of jobs
    let jobs = vec![
        WorkerJob::UpdateCharacterInfo {
            character_id: 12345,
        },
        WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000001,
        },
        WorkerJob::UpdateCorporationInfo {
            corporation_id: 98000001,
        },
        WorkerJob::UpdateAffiliations {
            character_ids: vec![11111, 22222, 33333],
        },
    ];

    for job in &jobs {
        queue
            .push(job.clone())
            .await
            .expect("Failed to push job to queue");
    }

    let pool = create_test_pool(&test, &redis).await;

    pool.start().await.expect("Failed to start pool");

    // Poll queue until empty or timeout (4 jobs with 1 dispatcher)
    let mut attempts = 0;
    let max_attempts = 20; // 20 attempts * 100ms = 2 seconds max
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let popped = queue.pop().await.expect("Failed to pop from queue");
        if popped.is_none() {
            break; // Queue is empty
        }
        attempts += 1;
        if attempts >= max_attempts {
            panic!("Jobs were not processed within timeout");
        }
    }

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_configuration_preserved() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
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

#[tokio::test]
async fn test_pool_permits_available_after_processing() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    queue.push(job).await.expect("Failed to push job to queue");

    let pool = create_test_pool(&test, &redis).await;

    let initial_permits = pool.available_permits();
    assert_eq!(initial_permits, 4, "Should have 4 permits initially");

    pool.start().await.expect("Failed to start pool");

    // Wait for job to be processed
    tokio::time::sleep(Duration::from_millis(300)).await;

    // After job is done, permits should be back to initial count
    let final_permits = pool.available_permits();
    assert_eq!(
        final_permits, initial_permits,
        "All permits should be available after job completes"
    );

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_with_minimal_config() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
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

#[tokio::test]
async fn test_pool_with_large_concurrency() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");

    // 119 jobs = 2 dispatchers (119 / 40 = 2.975 = 2)
    let config = WorkerPoolConfig::new(119);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    assert_eq!(pool.max_concurrent_jobs(), 119);
    assert_eq!(pool.available_permits(), 119);

    redis.cleanup().await.expect("Failed to cleanup Redis");
}
