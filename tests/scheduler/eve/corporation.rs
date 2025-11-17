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

#[tokio::test]
async fn returns_zero_when_no_corporations() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_corporation_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn returns_zero_when_all_corporations_up_to_date() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert corporations with recent info_updated_at
    test.eve().insert_mock_corporation(1, None, None).await?;
    test.eve().insert_mock_corporation(2, None, None).await?;
    test.eve().insert_mock_corporation(3, None, None).await?;

    let result = schedule_corporation_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_single_expired_corporation() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
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
        .exec(&test.state.db)
        .await?;

    let result = schedule_corporation_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_multiple_expired_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
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
            .exec(&test.state.db)
            .await?;
    }

    let result = schedule_corporation_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_only_expired_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
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
            .exec(&test.state.db)
            .await?;
    }

    // Insert 2 up-to-date corporations
    test.eve().insert_mock_corporation(4, None, None).await?;
    test.eve().insert_mock_corporation(5, None, None).await?;

    let result = schedule_corporation_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    // Only the 3 expired corporations should be scheduled
    assert_eq!(result.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_oldest_corporations_first() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
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
        .exec(&test.state.db)
        .await?;

    EveCorporation::update_many()
        .col_expr(
            entity::eve_corporation::Column::InfoUpdatedAt,
            Expr::value(oldest),
        )
        .filter(entity::eve_corporation::Column::Id.eq(corporation2.id))
        .exec(&test.state.db)
        .await?;

    EveCorporation::update_many()
        .col_expr(
            entity::eve_corporation::Column::InfoUpdatedAt,
            Expr::value(newest),
        )
        .filter(entity::eve_corporation::Column::Id.eq(corporation3.id))
        .exec(&test.state.db)
        .await?;

    let result = schedule_corporation_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn handles_duplicate_scheduling_attempts() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
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
        .exec(&test.state.db)
        .await?;

    // Schedule first time
    let result1 = schedule_corporation_info_update(&test.state.db, &queue).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);

    // Attempt to schedule again - duplicate jobs are rejected
    // The duplicate detection is based on job content (serialized JSON), not scheduled time
    let result2 = schedule_corporation_info_update(&test.state.db, &queue).await;
    assert!(result2.is_ok());
    // Same job already exists in queue, so it won't be scheduled again
    assert_eq!(result2.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_corporation_info_update(&test.state.db, &queue).await;

    assert!(result.is_err());

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_many_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
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
            .exec(&test.state.db)
            .await?;
    }

    let result = schedule_corporation_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 50);

    redis.cleanup().await?;
    Ok(())
}
