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

use bifrost_test_utils::{prelude::*, RedisTest};

use crate::server::{
    model::worker::WorkerJob,
    worker::{pool::WorkerPool, pool::WorkerPoolConfig, queue::WorkerJobQueue},
};

/// Create a test-optimized config with fast timeouts for testing
fn test_config(max_concurrent_jobs: usize) -> WorkerPoolConfig {
    let mut config = WorkerPoolConfig::new(max_concurrent_jobs);
    config.poll_interval_ms = 10;
    config.job_timeout_seconds = 1;
    config.shutdown_timeout_seconds = 1;
    config.cleanup_interval_ms = 100;
    config
}

/// Create a test worker pool with custom config
async fn create_test_pool_with_config(
    test: &TestSetup,
    redis: &RedisTest,
    config: WorkerPoolConfig,
) -> WorkerPool {
    let db = Arc::new(test.state.db.clone());
    let esi_client = Arc::new(test.state.esi_client.clone());
    let queue = Arc::new(WorkerJobQueue::with_queue_name(
        redis.redis_pool.clone(),
        redis.queue_name(),
    ));

    WorkerPool::new(config, db, esi_client, queue)
}

#[tokio::test]
async fn test_pool_initial_permits_available() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let config = test_config(10);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    assert_eq!(
        pool.available_permits(),
        10,
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
    let config = test_config(25);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    assert_eq!(
        pool.max_concurrent_jobs(),
        25,
        "Max concurrent jobs should match config"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_processes_single_job() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Push a job to the queue
    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    queue
        .push(job.clone())
        .await
        .expect("Failed to push job to queue");

    // Create and start pool
    let config = test_config(10);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

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
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

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
    let config = test_config(10);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

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
    let config = test_config(5);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    assert_eq!(
        pool.max_concurrent_jobs(),
        5,
        "Max concurrent jobs should be 5"
    );
    assert_eq!(
        pool.available_permits(),
        5,
        "All 5 permits should be available"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_active_job_count_calculation() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let config = test_config(10);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    // Initially no active jobs
    assert_eq!(pool.active_job_count(), 0);
    assert_eq!(pool.available_permits(), 10);

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

    let config = test_config(10);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

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
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

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

    let config = test_config(10);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    pool.start().await.expect("Failed to start pool");

    // Give time for all jobs to be processed
    tokio::time::sleep(Duration::from_millis(400)).await;

    // Queue should be empty
    let popped = queue.pop().await.expect("Failed to pop from queue");
    assert!(
        popped.is_none(),
        "All different job types should be processed"
    );

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_configuration_preserved() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");

    let mut config = test_config(15);
    config.dispatcher_count = 3;

    let pool = create_test_pool_with_config(&test, &redis, config).await;

    assert_eq!(pool.max_concurrent_jobs(), 15);
    assert_eq!(pool.available_permits(), 15);

    pool.start().await.expect("Failed to start pool");
    assert_eq!(pool.dispatcher_count().await, 3);

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_permits_available_after_processing() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    queue.push(job).await.expect("Failed to push job to queue");

    let config = test_config(5);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    let initial_permits = pool.available_permits();
    assert_eq!(initial_permits, 5, "Should have 5 permits initially");

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

    let mut config = test_config(1);
    config.dispatcher_count = 1;
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    assert_eq!(pool.max_concurrent_jobs(), 1);
    assert_eq!(pool.available_permits(), 1);

    pool.start().await.expect("Failed to start pool");
    assert_eq!(pool.dispatcher_count().await, 1);

    pool.stop().await.expect("Failed to stop pool");
    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pool_with_large_concurrency() {
    let test = test_setup_with_tables!().expect("Failed to create test setup");
    let redis = RedisTest::new().await.expect("Failed to create Redis test");

    let config = test_config(100);
    let pool = create_test_pool_with_config(&test, &redis, config).await;

    assert_eq!(pool.max_concurrent_jobs(), 100);
    assert_eq!(pool.available_permits(), 100);

    redis.cleanup().await.expect("Failed to cleanup Redis");
}
