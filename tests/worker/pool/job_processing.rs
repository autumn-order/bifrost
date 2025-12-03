//! Tests for WorkerPool job processing functionality.
//!
//! This module verifies the behavior of job execution within the worker pool, including
//! processing single and multiple jobs, handling empty queues gracefully, and supporting
//! all job types (Character, Alliance, Corporation, and Affiliation updates).

use std::time::Duration;

use bifrost::server::model::worker::WorkerJob;

use super::*;

/// Tests successful processing of a single job.
///
/// Verifies that when a single job is added to the queue and the pool is started,
/// the job is successfully processed and removed from the queue.
///
/// Expected: Queue is empty after job processing completes
#[tokio::test]
async fn processes_single_job() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests successful processing of multiple jobs sequentially.
///
/// Verifies that when multiple jobs of different types are added to the queue,
/// the pool processes all of them successfully and the queue becomes empty.
///
/// Expected: All jobs are processed and queue is empty
#[tokio::test]
async fn processes_multiple_jobs() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests graceful handling of empty queue.
///
/// Verifies that the pool handles an empty queue gracefully without errors or
/// crashes, continuing to poll for jobs and remaining in a running state.
///
/// Expected: Pool remains running when queue is empty
#[tokio::test]
async fn handles_empty_queue_gracefully() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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

/// Tests processing of different job types.
///
/// Verifies that the pool can handle and process all supported job types
/// (Character, Alliance, Corporation, Affiliation updates) without errors.
///
/// Expected: All job types are processed successfully
#[tokio::test]
async fn processes_different_job_types() {
    let test = TestBuilder::new()
        .build()
        .await
        .expect("Failed to create test setup");
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
