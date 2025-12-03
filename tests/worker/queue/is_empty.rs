//! Tests for WorkerQueue::is_empty method
//!
//! These tests verify the is_empty method's behavior including:
//! - Returns true for empty queues
//! - Returns false for non-empty queues
//! - Correct behavior after adding and removing jobs
//! - Consistency with len() method

use bifrost::server::model::worker::WorkerJob;

use crate::util::redis::RedisTest;

use super::setup_test_queue;

/// Tests that an empty queue returns true for is_empty.
///
/// Verifies that a newly created queue with no jobs returns true when
/// checking if it's empty, establishing the baseline behavior.
///
/// Expected: is_empty() returns true
#[tokio::test]
async fn test_is_empty_on_empty_queue() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let is_empty = queue.is_empty().await.expect("Should check if empty");
    assert!(is_empty, "Empty queue should return true for is_empty");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

/// Tests that is_empty returns false after pushing a job.
///
/// Verifies that after adding a job to the queue, is_empty correctly
/// returns false, indicating the queue contains at least one job.
///
/// Expected: is_empty() returns false after push
#[tokio::test]
async fn test_is_empty_after_push() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    queue.push(job).await.expect("Should push job");

    let is_empty = queue.is_empty().await.expect("Should check if empty");
    assert!(
        !is_empty,
        "Non-empty queue should return false for is_empty"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

/// Tests that is_empty returns true after popping all jobs.
///
/// Verifies that after pushing jobs to the queue and then popping all of them,
/// is_empty correctly returns true, indicating the queue is empty again.
///
/// Expected: is_empty() returns true after all jobs are popped
#[tokio::test]
async fn test_is_empty_after_pop_all() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Push 2 jobs
    for i in 1..=2 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue.push(job).await.expect("Should push job");
    }

    // Pop all jobs
    queue.pop().await.expect("Should pop job");
    queue.pop().await.expect("Should pop job");

    let is_empty = queue.is_empty().await.expect("Should check if empty");
    assert!(is_empty, "Queue should be empty after popping all jobs");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

/// Tests that is_empty returns false for scheduled jobs.
///
/// Verifies that a job scheduled for future execution is counted as present
/// in the queue, causing is_empty to return false even though the job is
/// not yet due for processing.
///
/// Expected: is_empty() returns false after scheduling a job
#[tokio::test]
async fn test_is_empty_after_schedule() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCorporationInfo {
        corporation_id: 98000001,
    };
    let scheduled_at = chrono::Utc::now() + chrono::Duration::seconds(3600);

    queue
        .schedule(job, scheduled_at)
        .await
        .expect("Should schedule job");

    let is_empty = queue.is_empty().await.expect("Should check if empty");
    assert!(
        !is_empty,
        "Queue should not be empty after scheduling a job"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

/// Tests that is_empty is consistent with len method.
///
/// Verifies that is_empty() always returns the same result as (len() == 0),
/// ensuring both methods accurately reflect the queue state. Tests both
/// empty and non-empty queue states for consistency.
///
/// Expected: is_empty() == (len() == 0) for both empty and non-empty queues
#[tokio::test]
async fn test_is_empty_consistency_with_len() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Empty queue - both should agree
    let is_empty = queue.is_empty().await.expect("Should check if empty");
    let len = queue.len().await.expect("Should get queue length");
    assert_eq!(
        is_empty,
        len == 0,
        "is_empty should match len == 0 for empty queue"
    );

    // Add a job
    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    queue.push(job).await.expect("Should push job");

    // Non-empty queue - both should agree
    let is_empty = queue.is_empty().await.expect("Should check if empty");
    let len = queue.len().await.expect("Should get queue length");
    assert_eq!(
        is_empty,
        len == 0,
        "is_empty should match len == 0 for non-empty queue"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}
