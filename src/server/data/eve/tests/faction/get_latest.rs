//! Tests for FactionRepository::get_latest method.
//!
//! This module verifies the behavior of retrieving the most recently updated
//! faction record from the database.

use super::*;

/// Tests retrieving the latest faction when one exists.
///
/// Verifies that the faction repository correctly returns the faction with
/// the most recent updated_at timestamp.
///
/// Expected: Ok(Some(faction)) with the latest faction
#[tokio::test]
async fn returns_latest_faction() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    // Insert first faction
    let _faction_1 = test.eve().insert_mock_faction(1).await?;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Insert second faction (should have newer timestamp)
    let faction_2 = test.eve().insert_mock_faction(2).await?;

    let repo = FactionRepository::new(&test.db);
    let result = repo.get_latest().await;

    assert!(result.is_ok());
    let latest = result.unwrap();
    assert!(latest.is_some());

    let latest_faction = latest.unwrap();
    assert_eq!(latest_faction.faction_id, faction_2.faction_id);
    assert!(latest_faction.updated_at >= faction_2.updated_at);

    Ok(())
}

/// Tests retrieving latest faction when database is empty.
///
/// Verifies that the faction repository returns None when no factions
/// exist in the database.
///
/// Expected: Ok(None)
#[tokio::test]
async fn returns_none_when_empty() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    let repo = FactionRepository::new(&test.db);
    let result = repo.get_latest().await;

    assert!(result.is_ok());
    let latest = result.unwrap();
    assert!(latest.is_none());

    Ok(())
}

/// Tests that get_latest returns the faction with newest timestamp.
///
/// Verifies that when multiple factions exist, the one with the most
/// recent updated_at is returned, not just the last inserted.
///
/// Expected: Ok(Some(faction)) with the faction that was most recently updated
#[tokio::test]
async fn returns_faction_with_newest_timestamp() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    // Insert three factions
    let faction_1 = test.eve().insert_mock_faction(1).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let faction_2 = test.eve().insert_mock_faction(2).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    test.eve().insert_mock_faction(3).await?;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Update faction_1's timestamp by re-upserting it
    let mock_faction_1 = test.eve().mock_faction(1);
    let repo = FactionRepository::new(&test.db);
    repo.upsert_many(vec![mock_faction_1]).await?;

    // Now faction_1 should have the newest timestamp
    let result = repo.get_latest().await;

    assert!(result.is_ok());
    let latest = result.unwrap();
    assert!(latest.is_some());

    let latest_faction = latest.unwrap();
    assert_eq!(latest_faction.faction_id, faction_1.faction_id);
    assert!(latest_faction.updated_at > faction_2.updated_at);

    Ok(())
}

/// Tests retrieving latest faction when only one exists.
///
/// Verifies that the method works correctly when there's only a single
/// faction in the database.
///
/// Expected: Ok(Some(faction)) with the only faction
#[tokio::test]
async fn returns_single_faction_when_only_one_exists() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    let faction = test.eve().insert_mock_faction(1).await?;

    let repo = FactionRepository::new(&test.db);
    let result = repo.get_latest().await;

    assert!(result.is_ok());
    let latest = result.unwrap();
    assert!(latest.is_some());

    let latest_faction = latest.unwrap();
    assert_eq!(latest_faction.faction_id, faction.faction_id);
    assert_eq!(latest_faction.id, faction.id);

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the faction repository returns an error when attempting
/// to retrieve the latest faction without the required tables being created.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let repo = FactionRepository::new(&test.db);
    let result = repo.get_latest().await;

    assert!(result.is_err());

    Ok(())
}

/// Tests that get_latest works after multiple updates.
///
/// Verifies that the method correctly tracks the most recent update across
/// multiple upsert operations on different factions.
///
/// Expected: Ok(Some(faction)) with the most recently updated faction
#[tokio::test]
async fn tracks_latest_across_multiple_updates() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    // Insert two factions
    let faction_1 = test.eve().insert_mock_faction(1).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    test.eve().insert_mock_faction(2).await?;

    // Get initial latest
    let repo = FactionRepository::new(&test.db);
    let initial_latest = repo.get_latest().await?.unwrap();
    assert_eq!(initial_latest.faction_id, 2);

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Update faction 1
    let mock_faction_1 = test.eve().mock_faction(1);
    let repo = FactionRepository::new(&test.db);
    repo.upsert_many(vec![mock_faction_1]).await?;

    // Now faction 1 should be latest
    let updated_latest = repo.get_latest().await?.unwrap();
    assert_eq!(updated_latest.faction_id, faction_1.faction_id);
    assert!(updated_latest.updated_at > initial_latest.updated_at);

    Ok(())
}
