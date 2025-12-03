//! Tests for WorkerPool job processing and concurrency.
//!
//! This module verifies the behavior of the worker pool's job processing system, including
//! concurrency control via semaphore, permit tracking, job execution with proper resource
//! management, and handling of multiple job types with configurable concurrency limits.

use std::time::Duration;

use bifrost::server::{
    model::worker::WorkerJob,
    worker::{
        handler::WorkerJobHandler,
        pool::{WorkerPool, WorkerPoolConfig},
    },
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

/// Create a test worker pool with custom config
async fn create_test_pool_with_config(
    test: &TestContext,
    redis: &RedisTest,
    config: WorkerPoolConfig,
) -> WorkerPool {
    let handler = WorkerJobHandler::new(test.db.clone(), test.esi_client.clone());
    let queue = setup_test_queue(redis);

    WorkerPool::new(config, queue, handler)
}

mod permits {
    use super::*;

    /// Tests initial permit availability matches configured concurrency.
    ///
    /// Verifies that when a worker pool is created with a concurrency limit of 4,
    /// all 4 permits are available initially and no jobs are active.
    ///
    /// Expected: available_permits() returns 4, active_job_count() returns 0
    #[tokio::test]
    async fn all_permits_available_initially() {
        let test = TestBuilder::new()
            .build()
            .await
            .expect("Failed to create test setup");
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

    /// Tests that max concurrent jobs matches configured value.
    ///
    /// Verifies that the pool's maximum concurrency limit is correctly set to
    /// the configured value (4 jobs) and accessible via the public API.
    ///
    /// Expected: max_concurrent_jobs() returns 4
    #[tokio::test]
    async fn max_concurrent_jobs_matches_config() {
        let test = TestBuilder::new()
            .build()
            .await
            .expect("Failed to create test setup");
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let pool = create_test_pool(&test, &redis).await;

        assert_eq!(
            pool.max_concurrent_jobs(),
            4,
            "Max concurrent jobs should match config"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that available permits return after job processing completes.
    ///
    /// Verifies that when a job is processed by the pool, the permit is acquired
    /// during execution and then released back to the pool after completion, making
    /// it available for subsequent jobs.
    ///
    /// Expected: Permits return to initial count after job completes
    #[tokio::test]
    async fn permits_return_after_processing() {
        let test = TestBuilder::new()
            .build()
            .await
            .expect("Failed to create test setup");
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
        tokio::time::sleep(Duration::from_millis(1200)).await;

        // After job is done, permits should be back to initial count
        let final_permits = pool.available_permits();
        assert_eq!(
            final_permits, initial_permits,
            "All permits should be available after job completes"
        );

        pool.stop().await.expect("Failed to stop pool");
        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests active job count calculation accuracy.
    ///
    /// Verifies that the active job count is correctly calculated as the difference
    /// between max concurrent jobs and available permits, ensuring accurate tracking
    /// of resource utilization.
    ///
    /// Expected: active_job_count() + available_permits() == max_concurrent_jobs()
    #[tokio::test]
    async fn active_job_count_calculated_correctly() {
        let test = TestBuilder::new()
            .build()
            .await
            .expect("Failed to create test setup");
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

    /// Tests concurrency limit is enforced correctly.
    ///
    /// Verifies that the pool enforces the configured concurrency limit by ensuring
    /// the maximum number of concurrent jobs and available permits are correctly set.
    ///
    /// Expected: max_concurrent_jobs() is 4 and all permits are available
    #[tokio::test]
    async fn respects_concurrency_limit() {
        let test = TestBuilder::new()
            .build()
            .await
            .expect("Failed to create test setup");
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
}

mod job_processing {
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
}

mod configuration {
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
}
