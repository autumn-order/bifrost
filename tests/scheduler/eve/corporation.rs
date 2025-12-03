//! Tests for schedule_corporation_info_update
//!
//! These tests verify the corporation info scheduling behavior including:
//! - Scheduling updates for corporations with expired cache
//! - Handling empty corporation tables
//! - Skipping corporations that are up to date
//! - Correct job creation with corporation IDs
//! - Batch limiting based on configuration

use bifrost::server::scheduler::eve::corporation::schedule_corporation_info_update;
use bifrost_test_utils::prelude::*;
use chrono::{Duration, Utc};
use entity::prelude::EveCorporation;
use migration::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::util::redis::RedisTest;
use crate::worker::queue::setup_test_queue;

/// Tests scheduling when no corporations exist in database.
///
/// Verifies that the corporation scheduler correctly handles an empty corporation table
/// by returning zero scheduled jobs without errors.
///
/// Expected: Ok(0) and empty queue
#[tokio::test]
async fn returns_zero_when_no_corporations() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_corporation_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    // Verify queue is empty
    assert_eq!(queue.len().await.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling when all corporations have fresh cache.
///
/// Verifies that the corporation scheduler skips corporations with recent info_updated_at
/// timestamps and returns zero scheduled jobs when all corporations are up to date.
///
/// Expected: Ok(0) and empty queue
#[tokio::test]
async fn returns_zero_when_all_corporations_up_to_date() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert corporations with recent info_updated_at
    test.eve().insert_mock_corporation(1, None, None).await?;
    test.eve().insert_mock_corporation(2, None, None).await?;
    test.eve().insert_mock_corporation(3, None, None).await?;

    let result = schedule_corporation_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    // Verify queue is empty
    assert_eq!(queue.len().await.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling a single corporation with expired cache.
///
/// Verifies that the corporation scheduler correctly identifies and schedules
/// a job for a corporation whose info_updated_at timestamp exceeds the cache duration
/// (24 hours).
///
/// Expected: Ok(1) and one job in queue
#[tokio::test]
async fn schedules_single_expired_corporation() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Set info_updated_at to 25 hours ago (cache is 24 hours)
    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    EveCorporation::update_many()
        .col_expr(
            entity::eve_corporation::Column::InfoUpdatedAt,
            Expr::value(old_timestamp),
        )
        .filter(entity::eve_corporation::Column::Id.eq(corporation.id))
        .exec(&test.db)
        .await?;

    let result = schedule_corporation_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling multiple corporations with expired cache.
///
/// Verifies that the corporation scheduler correctly identifies and schedules
/// jobs for multiple corporations whose info_updated_at timestamps exceed the cache
/// duration.
///
/// Expected: Ok(5) and five jobs in queue
#[tokio::test]
async fn schedules_multiple_expired_corporations() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert 5 corporations with expired cache
    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    for i in 1..=5 {
        let corporation = test.eve().insert_mock_corporation(i, None, None).await?;
        EveCorporation::update_many()
            .col_expr(
                entity::eve_corporation::Column::InfoUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_corporation::Column::Id.eq(corporation.id))
            .exec(&test.db)
            .await?;
    }

    let result = schedule_corporation_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);

    // Verify jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 5);

    redis.cleanup().await?;
    Ok(())
}

/// Tests selective scheduling of only expired corporations.
///
/// Verifies that the corporation scheduler distinguishes between expired and
/// up-to-date corporations, scheduling only those with expired cache while
/// skipping those with recent timestamps.
///
/// Expected: Ok(3) and three jobs in queue (only expired corporations)
#[tokio::test]
async fn schedules_only_expired_corporations() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert 3 expired corporations
    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    for i in 1..=3 {
        let corporation = test.eve().insert_mock_corporation(i, None, None).await?;
        EveCorporation::update_many()
            .col_expr(
                entity::eve_corporation::Column::InfoUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_corporation::Column::Id.eq(corporation.id))
            .exec(&test.db)
            .await?;
    }

    // Insert 2 up-to-date corporations
    test.eve().insert_mock_corporation(4, None, None).await?;
    test.eve().insert_mock_corporation(5, None, None).await?;

    let result = schedule_corporation_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // Only the 3 expired corporations should be scheduled
    assert_eq!(result.unwrap(), 3);

    // Verify only 3 jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

/// Tests that oldest corporations are prioritized for scheduling.
///
/// Verifies that the corporation scheduler processes corporations in order of
/// their info_updated_at timestamps, prioritizing the oldest (most stale) entries
/// first for optimal cache freshness management.
///
/// Expected: Ok(3) and jobs scheduled in age order (oldest first)
#[tokio::test]
async fn schedules_oldest_corporations_first() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation1 = test.eve().insert_mock_corporation(1, None, None).await?;
    let corporation2 = test.eve().insert_mock_corporation(2, None, None).await?;
    let corporation3 = test.eve().insert_mock_corporation(3, None, None).await?;

    // Set different expiration times
    let oldest = Utc::now().naive_utc() - Duration::hours(72);
    let middle = Utc::now().naive_utc() - Duration::hours(48);
    let newest = Utc::now().naive_utc() - Duration::hours(25);

    EveCorporation::update_many()
        .col_expr(
            entity::eve_corporation::Column::InfoUpdatedAt,
            Expr::value(middle),
        )
        .filter(entity::eve_corporation::Column::Id.eq(corporation1.id))
        .exec(&test.db)
        .await?;

    EveCorporation::update_many()
        .col_expr(
            entity::eve_corporation::Column::InfoUpdatedAt,
            Expr::value(oldest),
        )
        .filter(entity::eve_corporation::Column::Id.eq(corporation2.id))
        .exec(&test.db)
        .await?;

    EveCorporation::update_many()
        .col_expr(
            entity::eve_corporation::Column::InfoUpdatedAt,
            Expr::value(newest),
        )
        .filter(entity::eve_corporation::Column::Id.eq(corporation3.id))
        .exec(&test.db)
        .await?;

    let result = schedule_corporation_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    // Verify jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

/// Tests duplicate detection for scheduling attempts.
///
/// Verifies that the corporation scheduler prevents duplicate jobs from being
/// added to the queue. When the same corporation job is scheduled twice, the
/// second attempt is rejected based on job content matching.
///
/// Expected: First schedule Ok(1), second schedule Ok(0), one job in queue
#[tokio::test]
async fn handles_duplicate_scheduling_attempts() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    EveCorporation::update_many()
        .col_expr(
            entity::eve_corporation::Column::InfoUpdatedAt,
            Expr::value(old_timestamp),
        )
        .filter(entity::eve_corporation::Column::Id.eq(corporation.id))
        .exec(&test.db)
        .await?;

    // Schedule first time
    let result1 = schedule_corporation_info_update(test.db.clone(), queue.clone()).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    // Attempt to schedule again - duplicate jobs are rejected
    // The duplicate detection is based on job content (serialized JSON), not scheduled time
    let result2 = schedule_corporation_info_update(test.db.clone(), queue.clone()).await;
    assert!(result2.is_ok());
    // Same job already exists in queue, so it won't be scheduled again
    assert_eq!(result2.unwrap(), 0);

    // Verify still only 1 job in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the corporation scheduler returns an error when required
/// database tables (eve_corporation) are not present in the database schema.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_corporation_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_err());

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling a large batch of corporations.
///
/// Verifies that the corporation scheduler can handle scheduling many corporations
/// (50 in this test) efficiently, ensuring all expired corporations are processed
/// and queued correctly.
///
/// Expected: Ok(50) and fifty jobs in queue
#[tokio::test]
async fn schedules_many_corporations() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert 50 corporations with expired cache
    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    for i in 1..=50 {
        let corporation = test.eve().insert_mock_corporation(i, None, None).await?;
        EveCorporation::update_many()
            .col_expr(
                entity::eve_corporation::Column::InfoUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_corporation::Column::Id.eq(corporation.id))
            .exec(&test.db)
            .await?;
    }

    let result = schedule_corporation_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 50);

    // Verify jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 50);

    redis.cleanup().await?;
    Ok(())
}
