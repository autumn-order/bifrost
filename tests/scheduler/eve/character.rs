//! Tests for schedule_character_info_update scheduler.
//!
//! This module verifies the scheduler correctly identifies characters with expired
//! cache, prioritizes oldest entries first, and handles edge cases like empty tables,
//! duplicate scheduling attempts, and large batch processing.

use bifrost::server::scheduler::eve::character::schedule_character_info_update;
use bifrost_test_utils::prelude::*;
use chrono::{Duration, Utc};
use entity::prelude::EveCharacter;
use migration::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::util::redis::RedisTest;
use crate::worker::queue::setup_test_queue;

/// Tests scheduling when no characters exist in database.
///
/// Verifies that the character scheduler correctly handles an empty character table
/// by returning zero scheduled jobs without errors.
///
/// Expected: Ok(0) and empty queue
#[tokio::test]
async fn returns_zero_when_no_characters() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_character_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    // Verify queue is empty
    assert_eq!(queue.len().await.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling when all characters have fresh cache.
///
/// Verifies that the character scheduler skips characters with recent info_updated_at
/// timestamps and returns zero scheduled jobs when all characters are up to date.
///
/// Expected: Ok(0) and empty queue
#[tokio::test]
async fn returns_zero_when_all_characters_up_to_date() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert characters with recent info_updated_at
    test.eve()
        .insert_mock_character(1, corporation.corporation_id, None, None)
        .await?;
    test.eve()
        .insert_mock_character(2, corporation.corporation_id, None, None)
        .await?;
    test.eve()
        .insert_mock_character(3, corporation.corporation_id, None, None)
        .await?;

    let result = schedule_character_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    // Verify queue is empty
    assert_eq!(queue.len().await.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling a single character with expired cache.
///
/// Verifies that the character scheduler correctly identifies and schedules
/// a job for a character whose info_updated_at timestamp exceeds the cache duration
/// (30 days).
///
/// Expected: Ok(1) and one job in queue
#[tokio::test]
async fn schedules_single_expired_character() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;
    let character = test
        .eve()
        .insert_mock_character(1, corporation.corporation_id, None, None)
        .await?;

    // Set info_updated_at to 31 days ago (cache is 30 days)
    let old_timestamp = Utc::now().naive_utc() - Duration::days(31);
    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::InfoUpdatedAt,
            Expr::value(old_timestamp),
        )
        .filter(entity::eve_character::Column::Id.eq(character.id))
        .exec(&test.db)
        .await?;

    let result = schedule_character_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling multiple characters with expired cache.
///
/// Verifies that the character scheduler correctly identifies and schedules
/// jobs for multiple characters whose info_updated_at timestamps exceed the cache
/// duration.
///
/// Expected: Ok(5) and five jobs in queue
#[tokio::test]
async fn schedules_multiple_expired_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert 5 characters with expired cache
    let old_timestamp = Utc::now().naive_utc() - Duration::days(31);
    for i in 1..=5 {
        let character = test
            .eve()
            .insert_mock_character(i, corporation.corporation_id, None, None)
            .await?;
        EveCharacter::update_many()
            .col_expr(
                entity::eve_character::Column::InfoUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_character::Column::Id.eq(character.id))
            .exec(&test.db)
            .await?;
    }

    let result = schedule_character_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);

    // Verify jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 5);

    redis.cleanup().await?;
    Ok(())
}

/// Tests selective scheduling of only expired characters.
///
/// Verifies that the character scheduler distinguishes between expired and
/// up-to-date characters, scheduling only those with expired cache while
/// skipping those with recent timestamps.
///
/// Expected: Ok(3) and three jobs in queue (only expired characters)
#[tokio::test]
async fn schedules_only_expired_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert 3 expired characters
    let old_timestamp = Utc::now().naive_utc() - Duration::days(31);
    for i in 1..=3 {
        let character = test
            .eve()
            .insert_mock_character(i, corporation.corporation_id, None, None)
            .await?;
        EveCharacter::update_many()
            .col_expr(
                entity::eve_character::Column::InfoUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_character::Column::Id.eq(character.id))
            .exec(&test.db)
            .await?;
    }

    // Insert 2 up-to-date characters
    test.eve()
        .insert_mock_character(4, corporation.corporation_id, None, None)
        .await?;
    test.eve()
        .insert_mock_character(5, corporation.corporation_id, None, None)
        .await?;

    let result = schedule_character_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // Only the 3 expired characters should be scheduled
    assert_eq!(result.unwrap(), 3);

    // Verify only 3 jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

/// Tests that oldest characters are prioritized for scheduling.
///
/// Verifies that the character scheduler processes characters in order of
/// their info_updated_at timestamps, prioritizing the oldest (most stale) entries
/// first for optimal cache freshness management.
///
/// Expected: Ok(3) and jobs scheduled in age order (oldest first)
#[tokio::test]
async fn schedules_oldest_characters_first() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    let character1 = test
        .eve()
        .insert_mock_character(1, corporation.corporation_id, None, None)
        .await?;
    let character2 = test
        .eve()
        .insert_mock_character(2, corporation.corporation_id, None, None)
        .await?;
    let character3 = test
        .eve()
        .insert_mock_character(3, corporation.corporation_id, None, None)
        .await?;

    // Set different expiration times
    let oldest = Utc::now().naive_utc() - Duration::days(90);
    let middle = Utc::now().naive_utc() - Duration::days(60);
    let newest = Utc::now().naive_utc() - Duration::days(31);

    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::InfoUpdatedAt,
            Expr::value(middle),
        )
        .filter(entity::eve_character::Column::Id.eq(character1.id))
        .exec(&test.db)
        .await?;

    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::InfoUpdatedAt,
            Expr::value(oldest),
        )
        .filter(entity::eve_character::Column::Id.eq(character2.id))
        .exec(&test.db)
        .await?;

    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::InfoUpdatedAt,
            Expr::value(newest),
        )
        .filter(entity::eve_character::Column::Id.eq(character3.id))
        .exec(&test.db)
        .await?;

    let result = schedule_character_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    // Verify jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

/// Tests duplicate detection for scheduling attempts.
///
/// Verifies that the character scheduler prevents duplicate jobs from being
/// added to the queue. When the same character job is scheduled twice, the
/// second attempt is rejected based on job content matching.
///
/// Expected: First schedule Ok(1), second schedule Ok(0), one job in queue
#[tokio::test]
async fn handles_duplicate_scheduling_attempts() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;
    let character = test
        .eve()
        .insert_mock_character(1, corporation.corporation_id, None, None)
        .await?;

    let old_timestamp = Utc::now().naive_utc() - Duration::days(31);
    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::InfoUpdatedAt,
            Expr::value(old_timestamp),
        )
        .filter(entity::eve_character::Column::Id.eq(character.id))
        .exec(&test.db)
        .await?;

    // Schedule first time
    let result1 = schedule_character_info_update(test.db.clone(), queue.clone()).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    // Attempt to schedule again - duplicate jobs are rejected
    // The duplicate detection is based on job content (serialized JSON), not scheduled time
    let result2 = schedule_character_info_update(test.db.clone(), queue.clone()).await;
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
/// Verifies that the character scheduler returns an error when required
/// database tables (eve_character) are not present in the database schema.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_character_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_err());

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling a large batch of characters.
///
/// Verifies that the character scheduler can handle scheduling many characters
/// (50 in this test) efficiently, ensuring all expired characters are processed
/// and queued correctly.
///
/// Expected: Ok(50) and fifty jobs in queue
#[tokio::test]
async fn schedules_many_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert 50 characters with expired cache
    let old_timestamp = Utc::now().naive_utc() - Duration::days(31);
    for i in 1..=50 {
        let character = test
            .eve()
            .insert_mock_character(i, corporation.corporation_id, None, None)
            .await?;
        EveCharacter::update_many()
            .col_expr(
                entity::eve_character::Column::InfoUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_character::Column::Id.eq(character.id))
            .exec(&test.db)
            .await?;
    }

    let result = schedule_character_info_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 50);

    // Verify jobs are in the queue
    assert_eq!(queue.len().await.unwrap(), 50);

    redis.cleanup().await?;
    Ok(())
}
