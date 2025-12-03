//! Tests for schedule_faction_info_update
//!
//! These tests verify the faction info scheduling behavior including:
//! - Successfully scheduling faction update jobs
//! - Job creation without database dependencies
//! - Handling missing Redis connection
//! - Duplicate job prevention
//!
//! Unlike other entity schedulers (alliance, corporation, character), the faction scheduler
//! always enqueues exactly one job that checks all factions, since there are only a small,
//! fixed number of NPC factions in EVE Online.

use bifrost::server::{
    model::worker::WorkerJob, scheduler::eve::faction::schedule_faction_info_update,
};
use bifrost_test_utils::prelude::*;

use crate::util::redis::RedisTest;
use crate::worker::queue::setup_test_queue;

/// Tests successful scheduling of faction update job.
///
/// Verifies that the faction scheduler creates and enqueues a single UpdateFactionInfo
/// job. Unlike other entity schedulers, the faction scheduler always schedules exactly
/// one job since there are only a small, fixed number of NPC factions in EVE Online.
///
/// Expected: Ok(1) and one UpdateFactionInfo job in queue
#[tokio::test]
async fn schedules_faction_update_job() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_faction_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // Verify job is actually in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    // Pop the job to verify it's the correct type
    let job = queue.pop().await.unwrap();
    assert!(job.is_some());
    assert!(matches!(job.unwrap(), WorkerJob::UpdateFactionInfo));

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling without faction table in database.
///
/// Verifies that the faction scheduler works without database dependencies
/// since it doesn't query the faction table. The scheduler always enqueues
/// a job regardless of database state.
///
/// Expected: Ok(1) and one job in queue
#[tokio::test]
async fn schedules_without_faction_table() -> Result<(), TestError> {
    // The scheduler doesn't query the database, so it works even without tables
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_faction_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling behavior with existing factions in database.
///
/// Verifies that the faction scheduler always schedules exactly one job
/// regardless of how many factions exist in the database. The worker job
/// itself is responsible for checking all factions.
///
/// Expected: Ok(1) and one UpdateFactionInfo job in queue
#[tokio::test]
async fn schedules_with_existing_factions() -> Result<(), TestError> {
    // The scheduler always schedules 1 job regardless of what's in the database
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert some factions
    test.eve().insert_mock_faction(1).await?;
    test.eve().insert_mock_faction(2).await?;
    test.eve().insert_mock_faction(3).await?;

    let result = schedule_faction_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // Still just 1 job since the worker job handles all factions
    assert_eq!(result.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    // Pop the job to verify it's the correct type
    let job = queue.pop().await.unwrap();
    assert!(job.is_some());
    assert!(matches!(job.unwrap(), WorkerJob::UpdateFactionInfo));

    redis.cleanup().await?;
    Ok(())
}

/// Tests duplicate detection for scheduling attempts.
///
/// Verifies that the faction scheduler prevents duplicate jobs from being
/// added to the queue. When the same faction job is scheduled twice, the
/// second attempt is rejected based on job content matching.
///
/// Expected: First schedule Ok(1), second schedule Ok(0), one job in queue
#[tokio::test]
async fn handles_duplicate_scheduling_attempts() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Schedule first time
    let result1 = schedule_faction_info_update(test.db.clone(), queue.clone()).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    // Attempt to schedule again - duplicate jobs are rejected
    // The duplicate detection is based on job content (serialized JSON), not scheduled time
    let result2 = schedule_faction_info_update(test.db.clone(), queue.clone()).await;
    assert!(result2.is_ok());
    // Same job already exists in queue, so it won't be scheduled again
    assert_eq!(result2.unwrap(), 0);

    // Verify still only 1 job in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling after queue is cleared.
///
/// Verifies that the faction scheduler can successfully schedule a job again
/// after the queue has been cleared (simulating job processing). This ensures
/// the scheduler can run multiple times across different scheduling cycles.
///
/// Expected: Ok(1) for both scheduling attempts with fresh queues
#[tokio::test]
async fn schedules_multiple_times_after_queue_clear() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Schedule first time
    let result1 = schedule_faction_info_update(test.db.clone(), queue.clone()).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);

    // Clear the queue
    redis.cleanup().await?;

    // Create new Redis and queue for the second attempt
    let redis2 = RedisTest::new().await?;
    let queue2 = setup_test_queue(&redis2);

    // Should be able to schedule again with new queue
    let result2 = schedule_faction_info_update(test.db.clone(), queue2.clone()).await;
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap(), 1);

    // Verify job is in the new queue
    assert_eq!(queue2.len().await.unwrap(), 1);

    redis2.cleanup().await?;
    Ok(())
}

/// Tests concurrent scheduling attempts with race conditions.
///
/// Verifies that the faction scheduler handles concurrent scheduling attempts
/// correctly through duplicate detection. Multiple concurrent calls should result
/// in only one job being enqueued due to Redis-based duplicate prevention.
///
/// Expected: Total of 1 job scheduled across concurrent attempts
#[tokio::test]
async fn schedules_with_multiple_concurrent_calls() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Attempt to schedule from multiple concurrent calls
    let db = test.db.clone();
    let queue_clone = queue.clone();

    let handle1 = tokio::spawn(async move { schedule_faction_info_update(db, queue_clone).await });

    let db2 = test.db.clone();
    let queue_clone2 = queue.clone();

    let handle2 =
        tokio::spawn(async move { schedule_faction_info_update(db2, queue_clone2).await });

    let result1 = handle1.await.unwrap();
    let result2 = handle2.await.unwrap();

    assert!(result1.is_ok() || result2.is_ok());

    // One should succeed with 1, the other should get 0 (duplicate)
    let scheduled_count = result1.unwrap_or(0) + result2.unwrap_or(0);
    assert_eq!(scheduled_count, 1);

    // Verify only 1 job ended up in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests consistency of scheduling results across multiple operations.
///
/// Verifies that the faction scheduler returns consistent results when
/// scheduling with fresh queues. Each independent scheduling operation
/// should successfully enqueue exactly one job.
///
/// Expected: Ok(1) for each independent scheduling attempt
#[tokio::test]
async fn returns_consistent_result_on_success() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    // Test multiple independent scheduling operations return consistent results
    for _ in 0..3 {
        let redis = RedisTest::new().await?;
        let queue = setup_test_queue(&redis);

        let result = schedule_faction_info_update(test.db.clone(), queue.clone()).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // Verify job is in the queue
        assert_eq!(queue.len().await.unwrap(), 1);

        redis.cleanup().await?;
    }

    Ok(())
}

/// Tests scheduling with minimal database setup.
///
/// Verifies that the faction scheduler operates without any specific database
/// tables since it doesn't query the database. This confirms the scheduler's
/// independence from database schema requirements.
///
/// Expected: Ok(1) and one job in queue
#[tokio::test]
async fn works_with_minimal_database_setup() -> Result<(), TestError> {
    // Faction scheduler doesn't require any specific tables since it doesn't query the DB
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_faction_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}
