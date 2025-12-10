//! Tests for FactionRepository::update_all_timestamps method.
//!
//! This module verifies the behavior of updating timestamps for all faction
//! records when ESI returns 304 Not Modified.

use super::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// Tests updating timestamps for all factions.
///
/// Verifies that the faction repository successfully updates the updated_at
/// timestamp for all faction records in the database.
///
/// Expected: Ok with all faction timestamps updated
#[tokio::test]
async fn updates_timestamps_for_all_factions() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    // Insert multiple factions
    let faction_1 = test.eve().insert_mock_faction(1).await?;
    let faction_2 = test.eve().insert_mock_faction(2).await?;
    let faction_3 = test.eve().insert_mock_faction(3).await?;

    let old_timestamp_1 = faction_1.updated_at;
    let old_timestamp_2 = faction_2.updated_at;
    let old_timestamp_3 = faction_3.updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let repo = FactionRepository::new(&test.db);
    let result = repo.update_all_timestamps().await;

    assert!(result.is_ok());

    // Verify all timestamps were updated
    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions.len(), 3);

    for faction in factions {
        match faction.faction_id {
            _ if faction.faction_id == faction_1.faction_id => {
                assert!(faction.updated_at > old_timestamp_1);
            }
            _ if faction.faction_id == faction_2.faction_id => {
                assert!(faction.updated_at > old_timestamp_2);
            }
            _ if faction.faction_id == faction_3.faction_id => {
                assert!(faction.updated_at > old_timestamp_3);
            }
            _ => panic!("Unexpected faction_id: {}", faction.faction_id),
        }
    }

    Ok(())
}

/// Tests updating timestamps when database is empty.
///
/// Verifies that the faction repository handles the case when no factions
/// exist without errors.
///
/// Expected: Ok with no changes
#[tokio::test]
async fn handles_empty_database() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    let repo = FactionRepository::new(&test.db);
    let result = repo.update_all_timestamps().await;

    assert!(result.is_ok());

    // Verify no factions exist
    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions.len(), 0);

    Ok(())
}

/// Tests that update_all_timestamps only updates timestamp field.
///
/// Verifies that updating timestamps doesn't modify any other faction
/// data fields like name, description, etc.
///
/// Expected: Ok with all fields unchanged except updated_at
#[tokio::test]
async fn only_updates_timestamp_field() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    let faction = test.eve().insert_mock_faction(1).await?;
    let old_timestamp = faction.updated_at;
    let expected_name = faction.name.clone();
    let expected_description = faction.description.clone();
    let expected_created_at = faction.created_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let repo = FactionRepository::new(&test.db);
    repo.update_all_timestamps().await?;

    // Verify only updated_at changed
    let updated_faction = entity::prelude::EveFaction::find()
        .filter(entity::eve_faction::Column::FactionId.eq(faction.faction_id))
        .one(&test.db)
        .await?
        .unwrap();

    assert!(updated_faction.updated_at > old_timestamp);
    assert_eq!(updated_faction.name, expected_name);
    assert_eq!(updated_faction.description, expected_description);
    assert_eq!(updated_faction.created_at, expected_created_at);
    assert_eq!(updated_faction.faction_id, faction.faction_id);
    assert_eq!(updated_faction.corporation_id, faction.corporation_id);

    Ok(())
}

/// Tests updating timestamps for a single faction.
///
/// Verifies that the method works correctly when only one faction exists
/// in the database.
///
/// Expected: Ok with single faction timestamp updated
#[tokio::test]
async fn updates_timestamp_for_single_faction() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    let faction = test.eve().insert_mock_faction(1).await?;
    let old_timestamp = faction.updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let repo = FactionRepository::new(&test.db);
    let result = repo.update_all_timestamps().await;

    assert!(result.is_ok());

    // Verify timestamp was updated
    let updated_faction = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(updated_faction.updated_at > old_timestamp);

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the faction repository returns an error when attempting
/// to update timestamps without the required tables being created.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let repo = FactionRepository::new(&test.db);
    let result = repo.update_all_timestamps().await;

    assert!(result.is_err());

    Ok(())
}

/// Tests multiple consecutive timestamp updates.
///
/// Verifies that timestamps can be updated multiple times and each
/// update results in newer timestamps.
///
/// Expected: Ok with progressively newer timestamps on each update
#[tokio::test]
async fn handles_multiple_consecutive_updates() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    let faction = test.eve().insert_mock_faction(1).await?;
    let mut previous_timestamp = faction.updated_at;

    let repo = FactionRepository::new(&test.db);

    // Update timestamps 3 times
    for _ in 0..3 {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        repo.update_all_timestamps().await?;

        let updated = entity::prelude::EveFaction::find()
            .one(&test.db)
            .await?
            .unwrap();

        assert!(
            updated.updated_at > previous_timestamp,
            "Each update should result in a newer timestamp"
        );
        previous_timestamp = updated.updated_at;
    }

    Ok(())
}

/// Tests that all factions receive the same timestamp.
///
/// Verifies that when updating all timestamps, all factions receive
/// approximately the same timestamp (within a small window).
///
/// Expected: Ok with all faction timestamps very close to each other
#[tokio::test]
async fn all_factions_receive_similar_timestamp() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    // Insert multiple factions with different initial timestamps
    test.eve().insert_mock_faction(1).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    test.eve().insert_mock_faction(2).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    test.eve().insert_mock_faction(3).await?;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let repo = FactionRepository::new(&test.db);
    repo.update_all_timestamps().await?;

    // Get all factions and their timestamps
    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions.len(), 3);

    let timestamps: Vec<_> = factions.iter().map(|f| f.updated_at).collect();

    // All timestamps should be very close (within 1 second of each other)
    let min_timestamp = timestamps.iter().min().unwrap();
    let max_timestamp = timestamps.iter().max().unwrap();
    let difference = max_timestamp.signed_duration_since(*min_timestamp);

    assert!(
        difference.num_seconds() <= 1,
        "All timestamps should be updated at approximately the same time"
    );

    Ok(())
}
