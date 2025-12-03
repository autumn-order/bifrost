//! Tests for EntityRefreshTracker::find_entries_needing_update method.
//!
//! This module verifies the behavior of finding entries that need cache refresh based on
//! their updated_at timestamps. Tests cover empty tables, fresh cache detection, expired
//! entry identification, ordering by age, batch limits, and error handling.

use super::*;

/// Tests finding entries when database table is empty.
///
/// Verifies that the entity refresh tracker correctly handles an empty table
/// by returning an empty list without errors when no entries exist to update.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_when_no_entries() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

    assert!(result.is_ok());
    let ids = result.unwrap();
    assert!(ids.is_empty());

    Ok(())
}

/// Tests finding entries when all have fresh cache.
///
/// Verifies that the entity refresh tracker skips entries with recent updated_at
/// timestamps and returns an empty list when all entries are within cache duration.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_when_all_up_to_date() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    test.eve().insert_mock_alliance(1, None).await?;

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

    assert!(result.is_ok());
    let ids = result.unwrap();
    assert!(ids.is_empty());

    Ok(())
}

/// Tests finding entries with expired cache timestamps.
///
/// Verifies that the entity refresh tracker correctly identifies entries whose
/// updated_at timestamp exceeds the configured cache duration (24 hours for alliances)
/// and returns their IDs for scheduling.
///
/// Expected: Ok with Vec containing one alliance_id
#[tokio::test]
async fn returns_entries_with_expired_cache() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let alliance = test.eve().insert_mock_alliance(1, None).await?;

    // Update the alliance to have an old updated_at timestamp
    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    EveAlliance::update_many()
        .col_expr(
            entity::eve_alliance::Column::UpdatedAt,
            Expr::value(old_timestamp),
        )
        .filter(entity::eve_alliance::Column::Id.eq(alliance.id))
        .exec(&test.db)
        .await?;

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

    assert!(result.is_ok());
    let ids = result.unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], alliance.alliance_id);

    Ok(())
}

/// Tests that oldest entries are returned first.
///
/// Verifies that the entity refresh tracker orders entries by updated_at timestamp
/// in ascending order, prioritizing the oldest (most stale) entries first for
/// optimal cache freshness management.
///
/// Expected: Ok with Vec ordered by age (oldest to newest)
#[tokio::test]
async fn returns_oldest_updated_first() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
    let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

    // Set different updated_at timestamps
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

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

    assert!(result.is_ok());
    let ids = result.unwrap();
    assert_eq!(ids.len(), 3);
    // Should be ordered: alliance2 (oldest), alliance1 (middle), alliance3 (newest)
    assert_eq!(ids[0], alliance2.alliance_id);
    assert_eq!(ids[1], alliance1.alliance_id);
    assert_eq!(ids[2], alliance3.alliance_id);

    Ok(())
}

/// Tests batch limiting behavior.
///
/// Verifies that the entity refresh tracker respects batch size calculations
/// based on cache duration and schedule interval. With a 24h cache and 30min interval,
/// the minimum batch limit (100) is applied when the calculated batch would be smaller.
///
/// Expected: Ok with Vec containing all 10 entries (less than min limit of 100)
#[tokio::test]
async fn respects_batch_limit() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;

    // Create 10 alliances with expired cache
    let old_timestamp = Utc::now().naive_utc() - Duration::hours(25);
    for i in 1..=10 {
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

    // With 10 entries, cache 24h, interval 30min = 10 / 48 = 0 -> min 100
    // But we only have 10 entries, so we should get all 10
    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

    assert!(result.is_ok());
    let ids = result.unwrap();
    // Should return all 10 since that's less than MIN_BATCH_LIMIT (100)
    assert_eq!(ids.len(), 10);

    Ok(())
}

/// Tests finding a single expired entry.
///
/// Verifies that the entity refresh tracker correctly handles the case where
/// only one entry needs updating, returning a single-element list.
///
/// Expected: Ok with Vec containing one alliance_id
#[tokio::test]
async fn handles_single_entry() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
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

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

    assert!(result.is_ok());
    let ids = result.unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], alliance.alliance_id);

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the entity refresh tracker returns an error when required
/// database tables are not present in the database schema.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let result = tracker.find_entries_needing_update::<AllianceInfo>().await;

    assert!(result.is_err());

    Ok(())
}
