//! Tests for FactionRepository::upsert_many method.
//!
//! This module verifies the faction upsert behavior, including inserting new factions,
//! updating existing factions, and handling multiple factions at once.

use super::*;
use sea_orm::EntityTrait;

/// Tests upserting a new faction.
///
/// Verifies that the faction repository successfully inserts a new faction
/// record into the database.
///
/// Expected: Ok with Vec containing 1 created faction
#[tokio::test]
async fn upserts_new_faction() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;
    let mock_faction = test.eve().mock_faction(1);

    let repo = FactionRepository::new(&test.db);
    let result = repo.upsert_many(vec![mock_faction]).await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let created_factions = result.unwrap();
    assert_eq!(created_factions.len(), 1);

    Ok(())
}

/// Tests updating an existing faction.
///
/// Verifies that the faction repository updates an existing faction record when
/// upserting with the same faction ID, preserving created_at and updating
/// updated_at timestamp.
///
/// Expected: Ok with updated faction, preserved created_at, newer updated_at
#[tokio::test]
async fn updates_existing_faction() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;
    let mock_faction = test.eve().mock_faction(1);
    let mock_faction_update = test.eve().mock_faction(1);

    let repo = FactionRepository::new(&test.db);
    let initial = repo.upsert_many(vec![mock_faction]).await?;
    let initial_entry = initial.into_iter().next().expect("no entry returned");

    let initial_created_at = initial_entry.created_at;
    let initial_updated_at = initial_entry.updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let latest = repo.upsert_many(vec![mock_faction_update]).await?;
    let latest_entry = latest.into_iter().next().expect("no entry returned");

    // created_at should not change and updated_at should increase
    assert_eq!(latest_entry.created_at, initial_created_at);
    assert!(latest_entry.updated_at > initial_updated_at);

    Ok(())
}

/// Tests upserting multiple factions at once.
///
/// Verifies that the faction repository can insert multiple faction records
/// in a single operation.
///
/// Expected: Ok with Vec containing all created factions
#[tokio::test]
async fn upserts_multiple_factions() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;
    let faction_1 = test.eve().mock_faction(1);
    let faction_2 = test.eve().mock_faction(2);
    let faction_3 = test.eve().mock_faction(3);

    let repo = FactionRepository::new(&test.db);
    let result = repo
        .upsert_many(vec![faction_1, faction_2, faction_3])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let created_factions = result.unwrap();
    assert_eq!(created_factions.len(), 3);

    // Verify all faction IDs are present
    let faction_ids: Vec<i64> = created_factions.iter().map(|f| f.faction_id).collect();
    assert!(faction_ids.contains(&1));
    assert!(faction_ids.contains(&2));
    assert!(faction_ids.contains(&3));

    Ok(())
}

/// Tests upserting mixed new and existing factions.
///
/// Verifies that the faction repository correctly handles a batch containing
/// both new factions to insert and existing factions to update.
///
/// Expected: Ok with all factions created or updated
#[tokio::test]
async fn upserts_mixed_new_and_existing_factions() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;
    let faction_1 = test.eve().mock_faction(1);
    let faction_2 = test.eve().mock_faction(2);

    let repo = FactionRepository::new(&test.db);

    // Insert first two factions
    repo.upsert_many(vec![faction_1.clone(), faction_2.clone()])
        .await?;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Update faction 1 and insert faction 3
    let faction_1_update = test.eve().mock_faction(1);
    let faction_3 = test.eve().mock_faction(3);

    let repo = FactionRepository::new(&test.db);
    let result = repo.upsert_many(vec![faction_1_update, faction_3]).await;

    assert!(result.is_ok());
    let upserted = result.unwrap();
    assert_eq!(upserted.len(), 2);

    // Verify all 3 factions exist in database
    let all_factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(all_factions.len(), 3);

    Ok(())
}

/// Tests upserting with empty input.
///
/// Verifies that the faction repository handles empty input gracefully
/// without errors.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn handles_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    let repo = FactionRepository::new(&test.db);
    let result = repo.upsert_many(vec![]).await;

    assert!(result.is_ok());
    let factions = result.unwrap();
    assert!(factions.is_empty());

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the faction repository returns a database error when
/// attempting to upsert without the required tables being created.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let mut test = TestBuilder::new().build().await?;
    let mock_faction = test.eve().mock_faction(1);

    let repo = FactionRepository::new(&test.db);
    let result = repo.upsert_many(vec![mock_faction]).await;

    assert!(result.is_err());

    Ok(())
}

/// Tests that upsert preserves all faction fields.
///
/// Verifies that all faction fields are correctly stored and retrieved
/// after upserting.
///
/// Expected: Ok with all fields matching input data
#[tokio::test]
async fn preserves_all_faction_fields() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;
    let mock_faction = test.eve().mock_faction(1);
    let expected_name = mock_faction.name.clone();
    let expected_description = mock_faction.description.clone();

    let repo = FactionRepository::new(&test.db);
    let result = repo.upsert_many(vec![mock_faction]).await?;
    let created = &result[0];

    assert_eq!(created.name, expected_name);
    assert_eq!(created.description, expected_description);
    assert_eq!(created.faction_id, 1);

    Ok(())
}
