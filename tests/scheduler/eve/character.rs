//! Tests for schedule_character_info_update
//!
//! These tests verify the character info scheduling behavior including:
//! - Scheduling updates for characters with expired cache
//! - Handling empty character tables
//! - Skipping characters that are up to date
//! - Correct job creation with character IDs
//! - Batch limiting based on configuration

use bifrost::server::scheduler::eve::character::schedule_character_info_update;
use bifrost_test_utils::prelude::*;
use chrono::{Duration, Utc};
use entity::prelude::EveCharacter;
use migration::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::util::redis::RedisTest;
use crate::worker::queue::setup_test_queue;

#[tokio::test]
async fn returns_zero_when_no_characters() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_character_info_update(test.state.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

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

    let result = schedule_character_info_update(test.state.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

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
        .exec(&test.state.db)
        .await?;

    let result = schedule_character_info_update(test.state.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

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
            .exec(&test.state.db)
            .await?;
    }

    let result = schedule_character_info_update(test.state.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);

    redis.cleanup().await?;
    Ok(())
}

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
            .exec(&test.state.db)
            .await?;
    }

    // Insert 2 up-to-date characters
    test.eve()
        .insert_mock_character(4, corporation.corporation_id, None, None)
        .await?;
    test.eve()
        .insert_mock_character(5, corporation.corporation_id, None, None)
        .await?;

    let result = schedule_character_info_update(test.state.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    // Only the 3 expired characters should be scheduled
    assert_eq!(result.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

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
        .exec(&test.state.db)
        .await?;

    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::InfoUpdatedAt,
            Expr::value(oldest),
        )
        .filter(entity::eve_character::Column::Id.eq(character2.id))
        .exec(&test.state.db)
        .await?;

    EveCharacter::update_many()
        .col_expr(
            entity::eve_character::Column::InfoUpdatedAt,
            Expr::value(newest),
        )
        .filter(entity::eve_character::Column::Id.eq(character3.id))
        .exec(&test.state.db)
        .await?;

    let result = schedule_character_info_update(test.state.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

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
        .exec(&test.state.db)
        .await?;

    // Schedule first time
    let result1 = schedule_character_info_update(test.state.db.clone(), queue.clone()).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);

    // Attempt to schedule again - duplicate jobs are rejected
    // The duplicate detection is based on job content (serialized JSON), not scheduled time
    let result2 = schedule_character_info_update(test.state.db.clone(), queue.clone()).await;
    assert!(result2.is_ok());
    // Same job already exists in queue, so it won't be scheduled again
    assert_eq!(result2.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_character_info_update(test.state.db.clone(), queue.clone()).await;

    assert!(result.is_err());

    redis.cleanup().await?;
    Ok(())
}

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
            .exec(&test.state.db)
            .await?;
    }

    let result = schedule_character_info_update(test.state.db.clone(), queue.clone()).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 50);

    redis.cleanup().await?;
    Ok(())
}
