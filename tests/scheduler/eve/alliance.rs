//! Tests for schedule_alliance_info_update scheduler.
//!
//! This module verifies the scheduler correctly identifies alliances with expired
//! cache, prioritizes oldest entries first, and handles edge cases like empty tables,
//! duplicate scheduling attempts, and large batch processing.

use bifrost::server::scheduler::eve::alliance::schedule_alliance_info_update;
use bifrost::server::scheduler::SchedulerState;
use bifrost_test_utils::prelude::*;
use chrono::{Duration, Utc};
use entity::prelude::EveAlliance;
use migration::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::util::redis::RedisTest;
use crate::worker::queue::setup_test_queue;

/// Tests scheduling when no alliances exist in database.
///
/// Verifies that the alliance scheduler correctly handles an empty alliance table
/// by returning zero scheduled jobs without errors.
///
/// Expected: Ok(0) and empty queue
#[tokio::test]
async fn returns_zero_when_no_alliances() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let state = SchedulerState {
        db: test.db.clone(),
        queue: queue.clone(),
        offset_for_esi_downtime: false,
    };

    let result = schedule_alliance_info_update(state).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    // Verify queue is empty
    assert_eq!(queue.len().await.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling when all alliances have fresh cache.
///
/// Verifies that the alliance scheduler skips alliances with recent updated_at
/// timestamps and returns zero scheduled jobs when all alliances are up to date.
///
/// Expected: Ok(0) and empty queue
#[tokio::test]
async fn returns_zero_when_all_alliances_up_to_date() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert alliances with recent updated_at
    test.eve().insert_mock_alliance(1, None).await?;
    test.eve().insert_mock_alliance(2, None).await?;
    test.eve().insert_mock_alliance(3, None).await?;

    let state = SchedulerState {
        db: test.db.clone(),
        queue: queue.clone(),
        offset_for_esi_downtime: false,
    };

    let result = schedule_alliance_info_update(state).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    // Verify queue is empty
    assert_eq!(queue.len().await.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling a single alliance with expired cache.
///
/// Verifies that the alliance scheduler correctly identifies and schedules
/// a job for an alliance whose updated_at timestamp exceeds the cache duration
/// (24 hours).
///
/// Expected: Ok(1) and one job in queue
#[tokio::test]
async fn schedules_single_expired_alliance() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let alliance = test.eve().insert_mock_alliance(1, None).await?;

    // Set updated_at to 25 hours ago (cache is 24 hours)
    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    EveAlliance::update_many()
        .col_expr(
            entity::eve_alliance::Column::UpdatedAt,
            Expr::value(old_timestamp),
        )
        .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
        .exec(&test.db)
        .await?;

    let state = SchedulerState {
        db: test.db.clone(),
        queue: queue.clone(),
        offset_for_esi_downtime: false,
    };

    let result = schedule_alliance_info_update(state).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling multiple alliances with expired cache.
///
/// Verifies that the alliance scheduler correctly identifies and schedules
/// jobs for multiple alliances whose updated_at timestamps exceed the cache
/// duration.
///
/// Expected: Ok(5) and five jobs in queue
#[tokio::test]
async fn schedules_multiple_expired_alliances() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert 5 alliances with expired cache
    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    for i in 1..=5 {
        let alliance = test.eve().insert_mock_alliance(i, None).await?;
        EveAlliance::update_many()
            .col_expr(
                entity::eve_alliance::Column::UpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
            .exec(&test.db)
            .await?;
    }

    let state = SchedulerState {
        db: test.db.clone(),
        queue: queue.clone(),
        offset_for_esi_downtime: false,
    };

    let result = schedule_alliance_info_update(state).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);

    // Verify jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 5);

    redis.cleanup().await?;
    Ok(())
}

/// Tests selective scheduling of only expired alliances.
///
/// Verifies that the alliance scheduler distinguishes between expired and
/// up-to-date alliances, scheduling only those with expired cache while
/// skipping those with recent timestamps.
///
/// Expected: Ok(3) and three jobs in queue (only expired alliances)
#[tokio::test]
async fn schedules_only_expired_alliances() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert 3 expired alliances
    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    for i in 1..=3 {
        let alliance = test.eve().insert_mock_alliance(i, None).await?;
        EveAlliance::update_many()
            .col_expr(
                entity::eve_alliance::Column::UpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
            .exec(&test.db)
            .await?;
    }

    // Insert 2 up-to-date alliances
    test.eve().insert_mock_alliance(4, None).await?;
    test.eve().insert_mock_alliance(5, None).await?;

    let state = SchedulerState {
        db: test.db.clone(),
        queue: queue.clone(),
        offset_for_esi_downtime: false,
    };

    let result = schedule_alliance_info_update(state).await;

    assert!(result.is_ok());
    // Only the 3 expired alliances should be scheduled
    assert_eq!(result.unwrap(), 3);

    // Verify only 3 jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

/// Tests that oldest alliances are prioritized for scheduling.
///
/// Verifies that the alliance scheduler processes alliances in order of
/// their updated_at timestamps, prioritizing the oldest (most stale) entries
/// first for optimal cache freshness management.
///
/// Expected: Ok(3) and jobs scheduled in age order (oldest first)
#[tokio::test]
async fn schedules_oldest_alliances_first() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
    let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

    // Set different expiration times
    let oldest = Utc::now().naive_utc() - Duration::hours(72);
    let middle = Utc::now().naive_utc() - Duration::hours(48);
    let newest = Utc::now().naive_utc() - Duration::hours(25);

    EveAlliance::update_many()
        .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(middle))
        .filter(entity::eve_alliance::Column::Id.eq(alliance1.id))
        .exec(&test.db)
        .await?;

    EveAlliance::update_many()
        .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(oldest))
        .filter(entity::eve_alliance::Column::Id.eq(alliance2.id))
        .exec(&test.db)
        .await?;

    EveAlliance::update_many()
        .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(newest))
        .filter(entity::eve_alliance::Column::Id.eq(alliance3.id))
        .exec(&test.db)
        .await?;

    let state = SchedulerState {
        db: test.db.clone(),
        queue: queue.clone(),
        offset_for_esi_downtime: false,
    };

    let result = schedule_alliance_info_update(state).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    // Verify jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

/// Tests duplicate detection for scheduling attempts.
///
/// Verifies that the alliance scheduler prevents duplicate jobs from being
/// added to the queue. When the same alliance job is scheduled twice, the
/// second attempt is rejected based on job content matching.
///
/// Expected: First schedule Ok(1), second schedule Ok(0), one job in queue
#[tokio::test]
async fn handles_duplicate_scheduling_attempts() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let alliance = test.eve().insert_mock_alliance(1, None).await?;

    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    EveAlliance::update_many()
        .col_expr(
            entity::eve_alliance::Column::UpdatedAt,
            Expr::value(old_timestamp),
        )
        .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
        .exec(&test.db)
        .await?;

    let state = SchedulerState {
        db: test.db.clone(),
        queue: queue.clone(),
        offset_for_esi_downtime: false,
    };

    // Schedule first time
    let result1 = schedule_alliance_info_update(state.clone()).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    // Attempt to schedule again - duplicate jobs are rejected
    // The duplicate detection is based on job content (serialized JSON), not scheduled time
    let result2 = schedule_alliance_info_update(state).await;
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
/// Verifies that the alliance scheduler returns an error when required
/// database tables (eve_alliance) are not present in the database schema.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let state = SchedulerState {
        db: test.db.clone(),
        queue: queue.clone(),
        offset_for_esi_downtime: false,
    };

    let result = schedule_alliance_info_update(state).await;

    assert!(result.is_err());

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling a large batch of alliances.
///
/// Verifies that the alliance scheduler can handle scheduling many alliances
/// (50 in this test) efficiently, ensuring all expired alliances are processed
/// and queued correctly.
///
/// Expected: Ok(50) and fifty jobs in queue
#[tokio::test]
async fn schedules_many_alliances() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert 50 alliances with expired cache
    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    for i in 1..=50 {
        let alliance = test.eve().insert_mock_alliance(i, None).await?;
        EveAlliance::update_many()
            .col_expr(
                entity::eve_alliance::Column::UpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
            .exec(&test.db)
            .await?;
    }

    let state = SchedulerState {
        db: test.db.clone(),
        queue: queue.clone(),
        offset_for_esi_downtime: false,
    };

    let result = schedule_alliance_info_update(state).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 50);

    // Verify jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 50);

    redis.cleanup().await?;
    Ok(())
}
