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

use bifrost::server::scheduler::eve::faction::schedule_faction_info_update;
use bifrost_test_utils::prelude::*;

use crate::util::redis::RedisTest;
use crate::worker::queue::setup_test_queue;

#[tokio::test]
async fn schedules_faction_update_job() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_faction_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_without_faction_table() -> Result<(), TestError> {
    // The scheduler doesn't query the database, so it works even without tables
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_faction_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

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

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn handles_duplicate_scheduling_attempts() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Schedule first time
    let result1 = schedule_faction_info_update(test.db.clone(), queue.clone()).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);

    // Attempt to schedule again - duplicate jobs are rejected
    // The duplicate detection is based on job content (serialized JSON), not scheduled time
    let result2 = schedule_faction_info_update(test.db.clone(), queue.clone()).await;
    assert!(result2.is_ok());
    // Same job already exists in queue, so it won't be scheduled again
    assert_eq!(result2.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

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

    redis2.cleanup().await?;
    Ok(())
}

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

    redis.cleanup().await?;
    Ok(())
}

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

        redis.cleanup().await?;
    }

    Ok(())
}

#[tokio::test]
async fn works_with_minimal_database_setup() -> Result<(), TestError> {
    // Faction scheduler doesn't require any specific tables since it doesn't query the DB
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_faction_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}
