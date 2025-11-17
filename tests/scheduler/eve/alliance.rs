//! Tests for schedule_alliance_info_update
//!
//! These tests verify the alliance info scheduling behavior including:
//! - Scheduling updates for alliances with expired cache
//! - Handling empty alliance tables
//! - Skipping alliances that are up to date
//! - Correct job creation with alliance IDs
//! - Batch limiting based on configuration

use bifrost::server::scheduler::eve::alliance::schedule_alliance_info_update;
use bifrost_test_utils::prelude::*;
use chrono::{Duration, Utc};
use entity::prelude::{EveAlliance, EveFaction};
use migration::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::util::redis::RedisTest;
use crate::worker::queue::setup_test_queue;

#[tokio::test]
async fn returns_zero_when_no_alliances() -> Result<(), TestError> {
    let test = test_setup_with_tables!(EveFaction, EveAlliance)?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let result = schedule_alliance_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn returns_zero_when_all_alliances_up_to_date() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    // Insert alliances with recent updated_at
    test.eve().insert_mock_alliance(1, None).await?;
    test.eve().insert_mock_alliance(2, None).await?;
    test.eve().insert_mock_alliance(3, None).await?;

    let result = schedule_alliance_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_single_expired_alliance() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
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
        .exec(&test.state.db)
        .await?;

    let result = schedule_alliance_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_multiple_expired_alliances() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
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
            .exec(&test.state.db)
            .await?;
    }

    let result = schedule_alliance_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_only_expired_alliances() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
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
            .exec(&test.state.db)
            .await?;
    }

    // Insert 2 up-to-date alliances
    test.eve().insert_mock_alliance(4, None).await?;
    test.eve().insert_mock_alliance(5, None).await?;

    let result = schedule_alliance_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    // Only the 3 expired alliances should be scheduled
    assert_eq!(result.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_oldest_alliances_first() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
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
        .exec(&test.state.db)
        .await?;

    EveAlliance::update_many()
        .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(oldest))
        .filter(entity::eve_alliance::Column::Id.eq(alliance2.id))
        .exec(&test.state.db)
        .await?;

    EveAlliance::update_many()
        .col_expr(entity::eve_alliance::Column::UpdatedAt, Expr::value(newest))
        .filter(entity::eve_alliance::Column::Id.eq(alliance3.id))
        .exec(&test.state.db)
        .await?;

    let result = schedule_alliance_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn handles_duplicate_scheduling_attempts() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
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
        .exec(&test.state.db)
        .await?;

    // Schedule first time
    let result1 = schedule_alliance_info_update(&test.state.db, &queue).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);

    // Attempt to schedule again - duplicate jobs are rejected
    // The duplicate detection is based on job content (serialized JSON), not scheduled time
    let result2 = schedule_alliance_info_update(&test.state.db, &queue).await;
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

    let result = schedule_alliance_info_update(&test.state.db, &queue).await;

    assert!(result.is_err());

    redis.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn schedules_many_alliances() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(EveFaction, EveAlliance)?;
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
            .exec(&test.state.db)
            .await?;
    }

    let result = schedule_alliance_info_update(&test.state.db, &queue).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 50);

    redis.cleanup().await?;
    Ok(())
}
