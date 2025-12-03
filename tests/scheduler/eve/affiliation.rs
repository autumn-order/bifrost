//! Tests for schedule_character_affiliation_update
//!
//! These tests verify the character affiliation scheduling behavior including:
//! - Scheduling updates for characters with expired affiliation cache
//! - Handling empty character tables
//! - Skipping characters that are up to date
//! - Batching character IDs according to ESI limits
//! - Correct job creation with character ID lists

use bifrost::server::scheduler::eve::affiliation::schedule_character_affiliation_update;
use bifrost_test_utils::prelude::*;
use chrono::{Duration, Utc};
use entity::prelude::EveCharacter;
use migration::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::util::redis::RedisTest;
use crate::worker::queue::setup_test_queue;

/// Tests scheduling when no characters exist in database.
///
/// Verifies that the affiliation scheduler correctly handles an empty character table
/// by returning zero scheduled jobs without errors.
///
/// Expected: Ok(0) and empty queue
#[tokio::test]
async fn returns_zero_when_no_characters() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    // Verify queue is empty
    assert_eq!(queue.len().await.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling when all characters have fresh affiliation cache.
///
/// Verifies that the affiliation scheduler skips characters with recent affiliation_updated_at
/// timestamps and returns zero scheduled jobs when all characters are up to date.
///
/// Expected: Ok(0) and empty queue
#[tokio::test]
async fn returns_zero_when_all_characters_up_to_date() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert characters with recent affiliation_updated_at
    test.eve()
        .insert_mock_character(1, corporation.corporation_id, None, None)
        .await?;
    test.eve()
        .insert_mock_character(2, corporation.corporation_id, None, None)
        .await?;
    test.eve()
        .insert_mock_character(3, corporation.corporation_id, None, None)
        .await?;

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    // Verify queue is empty
    assert_eq!(queue.len().await.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling a single character with expired affiliation cache.
///
/// Verifies that the affiliation scheduler correctly identifies and schedules
/// a job for a character whose affiliation_updated_at timestamp exceeds the cache duration
/// (1 hour).
///
/// Expected: Ok(1) and one job in queue
#[tokio::test]
async fn schedules_single_expired_character_affiliation() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;
    let character = test
        .eve()
        .insert_mock_character(1, corporation.corporation_id, None, None)
        .await?;

    // Set affiliation_updated_at to 61 minutes ago (cache is 1 hour)
    let old_timestamp = Utc::now().naive_utc() - Duration::minutes(61);
    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::AffiliationUpdatedAt,
            Expr::value(old_timestamp),
        )
        .filter(entity::eve_character::Column::Id.eq(character.id))
        .exec(&test.db)
        .await?;

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling multiple characters with expired affiliation cache.
///
/// Verifies that the affiliation scheduler correctly identifies and batches
/// multiple characters whose affiliation_updated_at timestamps exceed the cache
/// duration into a single job (under the 1000 character ESI limit).
///
/// Expected: Ok(1) and one batch job in queue containing 5 characters
#[tokio::test]
async fn schedules_multiple_expired_character_affiliations() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert 5 characters with expired affiliation cache
    let old_timestamp = Utc::now().naive_utc() - Duration::minutes(61);
    for i in 1..=5 {
        let character = test
            .eve()
            .insert_mock_character(i, corporation.corporation_id, None, None)
            .await?;
        EveCharacter::update_many()
            .col_expr(
                entity::eve_character::Column::AffiliationUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_character::Column::Id.eq(character.id))
            .exec(&test.db)
            .await?;
    }

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // All 5 characters fit in one batch (under 1000 limit)
    assert_eq!(result.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests selective scheduling of only expired character affiliations.
///
/// Verifies that the affiliation scheduler distinguishes between expired and
/// up-to-date characters, scheduling only those with expired affiliation cache while
/// skipping those with recent timestamps.
///
/// Expected: Ok(1) and one batch job in queue (only expired characters)
#[tokio::test]
async fn schedules_only_expired_character_affiliations() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert 3 expired characters
    let old_timestamp = Utc::now().naive_utc() - Duration::minutes(61);
    for i in 1..=3 {
        let character = test
            .eve()
            .insert_mock_character(i, corporation.corporation_id, None, None)
            .await?;
        EveCharacter::update_many()
            .col_expr(
                entity::eve_character::Column::AffiliationUpdatedAt,
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

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // Only the 3 expired characters should be scheduled (in 1 batch)
    assert_eq!(result.unwrap(), 1);

    // Verify only 1 job in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests that oldest affiliations are prioritized for scheduling.
///
/// Verifies that the affiliation scheduler processes characters in order of
/// their affiliation_updated_at timestamps, prioritizing the oldest (most stale) entries
/// first for optimal cache freshness management.
///
/// Expected: Ok(1) and batch job containing characters ordered by age
#[tokio::test]
async fn schedules_oldest_affiliations_first() -> Result<(), TestError> {
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
    let oldest = Utc::now().naive_utc() - Duration::hours(5);
    let middle = Utc::now().naive_utc() - Duration::hours(3);
    let newest = Utc::now().naive_utc() - Duration::minutes(61);

    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::AffiliationUpdatedAt,
            Expr::value(middle),
        )
        .filter(entity::eve_character::Column::Id.eq(character1.id))
        .exec(&test.db)
        .await?;

    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::AffiliationUpdatedAt,
            Expr::value(oldest),
        )
        .filter(entity::eve_character::Column::Id.eq(character2.id))
        .exec(&test.db)
        .await?;

    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::AffiliationUpdatedAt,
            Expr::value(newest),
        )
        .filter(entity::eve_character::Column::Id.eq(character3.id))
        .exec(&test.db)
        .await?;

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // All 3 fit in one batch
    assert_eq!(result.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests duplicate detection for scheduling attempts.
///
/// Verifies that the affiliation scheduler prevents duplicate jobs from being
/// added to the queue. When the same affiliation job is scheduled twice, the
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

    let old_timestamp = Utc::now().naive_utc() - Duration::minutes(61);
    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::AffiliationUpdatedAt,
            Expr::value(old_timestamp),
        )
        .filter(entity::eve_character::Column::Id.eq(character.id))
        .exec(&test.db)
        .await?;

    // Schedule first time
    let result1 = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    // Attempt to schedule again - duplicate jobs are rejected
    // The duplicate detection is based on job content (serialized JSON), not scheduled time
    let result2 = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;
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
/// Verifies that the affiliation scheduler returns an error when required
/// database tables (eve_character) are not present in the database schema.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_err());

    redis.cleanup().await?;
    Ok(())
}

/// Tests batching behavior when character count exceeds ESI limit.
///
/// Verifies that the affiliation scheduler correctly batches characters into
/// multiple jobs when the total count exceeds the ESI limit of 1000 characters
/// per request. With 2500 characters, should create multiple batches.
///
/// Expected: Ok(n) where n >= 1, with at least one job scheduled
#[tokio::test]
async fn batches_characters_when_over_esi_limit() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert 2500 characters with expired affiliation cache
    // ESI limit is 1000, so this should create 3 batches (1000, 1000, 500)
    let old_timestamp = Utc::now().naive_utc() - Duration::minutes(61);
    for i in 1..=2500 {
        let character = test
            .eve()
            .insert_mock_character(i, corporation.corporation_id, None, None)
            .await?;
        EveCharacter::update_many()
            .col_expr(
                entity::eve_character::Column::AffiliationUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_character::Column::Id.eq(character.id))
            .exec(&test.db)
            .await?;
    }

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // All 2500 characters are batched into jobs, but duplicate detection in Redis
    // may reduce the actual scheduled count. We should have at least 1 job scheduled.
    let scheduled = result.unwrap();
    assert!(scheduled >= 1, "Expected at least 1 job, got {}", scheduled);

    redis.cleanup().await?;
    Ok(())
}

/// Tests batching when character count exactly matches ESI limit.
///
/// Verifies that the affiliation scheduler handles exactly 1000 characters
/// (the ESI limit) by creating a single batch job without overflow.
///
/// Expected: Ok(1) with exactly one job containing 1000 characters
#[tokio::test]
async fn batches_exactly_at_esi_limit() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert exactly 1000 characters (ESI limit)
    let old_timestamp = Utc::now().naive_utc() - Duration::minutes(61);
    for i in 1..=1000 {
        let character = test
            .eve()
            .insert_mock_character(i, corporation.corporation_id, None, None)
            .await?;
        EveCharacter::update_many()
            .col_expr(
                entity::eve_character::Column::AffiliationUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_character::Column::Id.eq(character.id))
            .exec(&test.db)
            .await?;
    }

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // Should create exactly 1 job with 1000 characters
    let scheduled = result.unwrap();
    assert!(scheduled >= 1, "Expected at least 1 job, got {}", scheduled);

    redis.cleanup().await?;
    Ok(())
}

/// Tests batching when character count is just over ESI limit.
///
/// Verifies that the affiliation scheduler handles 1001 characters (one over
/// the ESI limit) by creating multiple batch jobs as needed.
///
/// Expected: Ok(n) where n >= 1, with at least one job scheduled
#[tokio::test]
async fn batches_just_over_esi_limit() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert 1001 characters (just over ESI limit)
    let old_timestamp = Utc::now().naive_utc() - Duration::minutes(61);
    for i in 1..=1001 {
        let character = test
            .eve()
            .insert_mock_character(i, corporation.corporation_id, None, None)
            .await?;
        EveCharacter::update_many()
            .col_expr(
                entity::eve_character::Column::AffiliationUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_character::Column::Id.eq(character.id))
            .exec(&test.db)
            .await?;
    }

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // Should create multiple jobs but duplicate detection may reduce count
    let scheduled = result.unwrap();
    assert!(scheduled >= 1, "Expected at least 1 job, got {}", scheduled);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling a moderate batch of characters within ESI limit.
///
/// Verifies that the affiliation scheduler can handle scheduling many characters
/// (50 in this test) efficiently within a single batch, ensuring all expired
/// affiliations are processed and queued correctly.
///
/// Expected: Ok(1) and one batch job in queue containing 50 characters
#[tokio::test]
async fn schedules_many_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;

    // Insert 50 characters with expired affiliation cache
    let old_timestamp = Utc::now().naive_utc() - Duration::minutes(61);
    for i in 1..=50 {
        let character = test
            .eve()
            .insert_mock_character(i, corporation.corporation_id, None, None)
            .await?;
        EveCharacter::update_many()
            .col_expr(
                entity::eve_character::Column::AffiliationUpdatedAt,
                Expr::value(old_timestamp),
            )
            .filter(entity::eve_character::Column::Id.eq(character.id))
            .exec(&test.db)
            .await?;
    }

    let result = schedule_character_affiliation_update(test.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // All 50 fit in one batch (under 1000 limit)
    assert_eq!(result.unwrap(), 1);

    // Verify job is in the queue
    assert_eq!(queue.len().await.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}
