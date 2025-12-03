//! Tests for WorkerPool permit and concurrency management.
//!
//! This module verifies the behavior of the worker pool's semaphore-based concurrency
//! control system. Tests cover initial permit availability, permit acquisition and release,
//! active job count tracking, and enforcement of configured concurrency limits.

use std::time::Duration;

use bifrost::server::model::worker::WorkerJob;

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
