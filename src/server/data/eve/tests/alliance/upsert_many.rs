//! Tests for AllianceRepository::upsert_many method.
//!
//! This module verifies the alliance upsert behavior, including inserting new
//! alliances, updating existing alliances, handling mixed batches, faction
//! relationships, and large batch operations.

use super::*;

/// Tests upserting new alliances.
///
/// Verifies that the alliance repository successfully inserts new alliance
/// records into the database.
///
/// Expected: Ok with Vec containing 2 created alliances
#[tokio::test]
async fn upserts_new_alliances() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let (alliance_id_1, alliance_1) = test.eve().mock_alliance(1, None);
    let (alliance_id_2, alliance_2) = test.eve().mock_alliance(2, None);

    let alliance_repo = AllianceRepository::new(&test.db);
    let result = alliance_repo
        .upsert_many(vec![
            (alliance_id_1, alliance_1, None),
            (alliance_id_2, alliance_2, None),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let created_alliances = result.unwrap();
    assert_eq!(created_alliances.len(), 2);

    Ok(())
}

/// Tests updating existing alliances.
///
/// Verifies that the alliance repository updates existing alliance records when
/// upserting with the same alliance IDs, preserving created_at and updating
/// updated_at timestamps and modified fields.
///
/// Expected: Ok with updated alliances, preserved created_at, newer updated_at
#[tokio::test]
async fn updates_existing_alliances() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let (alliance_id_1, alliance_1) = test.eve().mock_alliance(1, None);
    let (alliance_id_2, alliance_2) = test.eve().mock_alliance(2, None);
    let (alliance_id_1_update, mut alliance_1_update) = test.eve().mock_alliance(1, None);
    let (alliance_id_2_update, mut alliance_2_update) = test.eve().mock_alliance(2, None);

    let alliance_repo = AllianceRepository::new(&test.db);
    let initial = alliance_repo
        .upsert_many(vec![
            (alliance_id_1, alliance_1, None),
            (alliance_id_2, alliance_2, None),
        ])
        .await?;

    let initial_entry_1 = initial
        .iter()
        .find(|a| a.alliance_id == alliance_id_1)
        .expect("alliance 1 not found");
    let initial_entry_2 = initial
        .iter()
        .find(|a| a.alliance_id == alliance_id_2)
        .expect("alliance 2 not found");

    let initial_created_at_1 = initial_entry_1.created_at;
    let initial_updated_at_1 = initial_entry_1.updated_at;
    let initial_created_at_2 = initial_entry_2.created_at;
    let initial_updated_at_2 = initial_entry_2.updated_at;

    // Modify alliance data to verify updates
    alliance_1_update.name = "Updated Alliance 1".to_string();
    alliance_2_update.name = "Updated Alliance 2".to_string();

    let latest = alliance_repo
        .upsert_many(vec![
            (alliance_id_1_update, alliance_1_update, None),
            (alliance_id_2_update, alliance_2_update, None),
        ])
        .await?;

    let latest_entry_1 = latest
        .iter()
        .find(|a| a.alliance_id == alliance_id_1_update)
        .expect("alliance 1 not found");
    let latest_entry_2 = latest
        .iter()
        .find(|a| a.alliance_id == alliance_id_2_update)
        .expect("alliance 2 not found");

    // created_at should not change and updated_at should increase for both alliances
    assert_eq!(latest_entry_1.created_at, initial_created_at_1);
    assert!(latest_entry_1.updated_at > initial_updated_at_1);
    assert_eq!(latest_entry_1.name, "Updated Alliance 1");
    assert_eq!(latest_entry_2.created_at, initial_created_at_2);
    assert!(latest_entry_2.updated_at > initial_updated_at_2);
    assert_eq!(latest_entry_2.name, "Updated Alliance 2");

    Ok(())
}

/// Tests upserting mixed new and existing alliances.
///
/// Verifies that the alliance repository correctly handles a batch containing
/// both new alliances (to insert) and existing alliances (to update) in a
/// single operation.
///
/// Expected: Ok with Vec containing both updated and newly created alliances
#[tokio::test]
async fn upserts_mixed_new_and_existing_alliances() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let (alliance_id_1, alliance_1) = test.eve().mock_alliance(1, None);
    let (alliance_id_2, alliance_2) = test.eve().mock_alliance(2, None);
    let (alliance_id_3, alliance_3) = test.eve().mock_alliance(3, None);
    let (alliance_id_1_update, mut alliance_1_update) = test.eve().mock_alliance(1, None);
    alliance_1_update.name = "Updated Alliance 1".to_string();
    let (alliance_id_2_update, alliance_2_update) = test.eve().mock_alliance(2, None);

    let alliance_repo = AllianceRepository::new(&test.db);

    // First, insert alliances 1 and 2
    let initial = alliance_repo
        .upsert_many(vec![
            (alliance_id_1, alliance_1, None),
            (alliance_id_2, alliance_2, None),
        ])
        .await?;

    assert_eq!(initial.len(), 2);
    let initial_alliance_1 = initial
        .iter()
        .find(|a| a.alliance_id == alliance_id_1)
        .expect("alliance 1 not found");
    let initial_created_at = initial_alliance_1.created_at;

    // Now upsert alliances 1 (update), 2 (update), and 3 (new)
    let result = alliance_repo
        .upsert_many(vec![
            (alliance_id_1_update, alliance_1_update, None),
            (alliance_id_2_update, alliance_2_update, None),
            (alliance_id_3, alliance_3, None),
        ])
        .await?;

    assert_eq!(result.len(), 3);

    let updated_alliance_1 = result
        .iter()
        .find(|a| a.alliance_id == alliance_id_1)
        .expect("alliance 1 not found");
    let alliance_3 = result
        .iter()
        .find(|a| a.alliance_id == alliance_id_3)
        .expect("alliance 3 not found");

    // Alliance 1 should be updated (same created_at, changed name)
    assert_eq!(updated_alliance_1.created_at, initial_created_at);
    assert_eq!(updated_alliance_1.name, "Updated Alliance 1");

    // Alliance 3 should be newly created
    assert_eq!(alliance_3.alliance_id, alliance_id_3);

    Ok(())
}

/// Tests handling empty input.
///
/// Verifies that the alliance repository handles empty upsert lists gracefully
/// without errors.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn handles_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;

    let alliance_repo = AllianceRepository::new(&test.db);
    let result = alliance_repo.upsert_many(vec![]).await?;

    assert_eq!(result.len(), 0);

    Ok(())
}

/// Tests upserting alliances with faction relationships.
///
/// Verifies that the alliance repository correctly handles alliances with
/// various faction relationships (Some faction, different factions, None).
///
/// Expected: Ok with alliances having correct faction_id assignments
#[tokio::test]
async fn upserts_with_faction_relationships() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let faction_1 = test.eve().insert_mock_faction(1).await?;
    let faction_2 = test.eve().insert_mock_faction(2).await?;

    let (alliance_id_1, alliance_1) = test.eve().mock_alliance(1, Some(1));
    let (alliance_id_2, alliance_2) = test.eve().mock_alliance(2, Some(2));
    let (alliance_id_3, alliance_3) = test.eve().mock_alliance(3, None);

    let alliance_repo = AllianceRepository::new(&test.db);
    let result = alliance_repo
        .upsert_many(vec![
            (alliance_id_1, alliance_1, Some(faction_1.id)),
            (alliance_id_2, alliance_2, Some(faction_2.id)),
            (alliance_id_3, alliance_3, None),
        ])
        .await?;

    assert_eq!(result.len(), 3);

    let alliance_1 = result
        .iter()
        .find(|a| a.alliance_id == alliance_id_1)
        .unwrap();
    let alliance_2 = result
        .iter()
        .find(|a| a.alliance_id == alliance_id_2)
        .unwrap();
    let alliance_3 = result
        .iter()
        .find(|a| a.alliance_id == alliance_id_3)
        .unwrap();

    assert_eq!(alliance_1.faction_id, Some(faction_1.id));
    assert_eq!(alliance_2.faction_id, Some(faction_2.id));
    assert_eq!(alliance_3.faction_id, None);

    Ok(())
}

/// Tests updating faction relationships.
///
/// Verifies that the alliance repository correctly updates faction relationships
/// when an alliance is upserted with a different faction than it previously had.
///
/// Expected: Ok with alliance having updated faction_id
#[tokio::test]
async fn updates_faction_relationships() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let faction_1 = test.eve().insert_mock_faction(1).await?;
    let faction_2 = test.eve().insert_mock_faction(2).await?;

    let (alliance_id, alliance) = test.eve().mock_alliance(1, Some(1));
    let (alliance_id_update, alliance_update) = test.eve().mock_alliance(1, Some(2));

    let alliance_repo = AllianceRepository::new(&test.db);

    // Insert with faction 1
    let initial = alliance_repo
        .upsert_many(vec![(alliance_id, alliance, Some(faction_1.id))])
        .await?;

    assert_eq!(initial[0].faction_id, Some(faction_1.id));

    // Update to faction 2
    let updated = alliance_repo
        .upsert_many(vec![(
            alliance_id_update,
            alliance_update,
            Some(faction_2.id),
        )])
        .await?;

    assert_eq!(updated[0].faction_id, Some(faction_2.id));
    assert_eq!(updated[0].alliance_id, alliance_id);

    Ok(())
}

/// Tests removing faction relationships.
///
/// Verifies that the alliance repository correctly removes faction relationships
/// when an alliance that previously had a faction is upserted with None.
///
/// Expected: Ok with alliance having None for faction_id
#[tokio::test]
async fn removes_faction_relationships() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let faction = test.eve().insert_mock_faction(1).await?;

    let (alliance_id, alliance) = test.eve().mock_alliance(1, Some(1));
    let (alliance_id_update, alliance_update) = test.eve().mock_alliance(1, None);

    let alliance_repo = AllianceRepository::new(&test.db);

    // Insert with faction
    let initial = alliance_repo
        .upsert_many(vec![(alliance_id, alliance, Some(faction.id))])
        .await?;

    assert_eq!(initial[0].faction_id, Some(faction.id));

    // Update to remove faction
    let updated = alliance_repo
        .upsert_many(vec![(alliance_id_update, alliance_update, None)])
        .await?;

    assert_eq!(updated[0].faction_id, None);
    assert_eq!(updated[0].alliance_id, alliance_id);

    Ok(())
}

/// Tests handling large batch upsert.
///
/// Verifies that the alliance repository efficiently handles upserting large
/// batches of alliances (100 in this test) in a single operation.
///
/// Expected: Ok with Vec containing all 100 alliances
#[tokio::test]
async fn handles_large_batch() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;

    let mut alliances = Vec::new();
    for i in 1..=100 {
        let (alliance_id, alliance) = test.eve().mock_alliance(i, None);
        alliances.push((alliance_id, alliance, None));
    }

    let alliance_repo = AllianceRepository::new(&test.db);
    let result = alliance_repo.upsert_many(alliances).await?;

    assert_eq!(result.len(), 100);

    // Verify all alliance IDs are present
    for i in 1..=100 {
        assert!(result.iter().any(|a| a.alliance_id == i));
    }

    Ok(())
}

/// Tests that ticker is correctly stored and updated.
///
/// Verifies that the alliance ticker field is properly persisted and updated
/// when alliances are upserted.
///
/// Expected: Ok with alliances having correct ticker values
#[tokio::test]
async fn stores_and_updates_ticker() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let (alliance_id, mut alliance) = test.eve().mock_alliance(1, None);
    alliance.ticker = "TEST".to_string();

    let (alliance_id_update, mut alliance_update) = test.eve().mock_alliance(1, None);
    alliance_update.ticker = "NEW".to_string();

    let alliance_repo = AllianceRepository::new(&test.db);
    let created = alliance_repo
        .upsert_many(vec![(alliance_id, alliance, None)])
        .await?;

    assert_eq!(created[0].ticker, "TEST");

    // Update ticker

    let updated = alliance_repo
        .upsert_many(vec![(alliance_id_update, alliance_update, None)])
        .await?;

    assert_eq!(updated[0].ticker, "NEW");
    assert_eq!(updated[0].alliance_id, alliance_id);

    Ok(())
}

/// Tests that executor corporation is correctly stored.
///
/// Verifies that the executor corporation field is properly persisted for
/// alliances with and without executor corporations.
///
/// Expected: Ok with alliances having correct executor_corporation_id values
#[tokio::test]
async fn stores_executor_corporation() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let (alliance_id_1, mut alliance_1) = test.eve().mock_alliance(1, None);
    alliance_1.executor_corporation_id = Some(1000);

    let (alliance_id_2, mut alliance_2) = test.eve().mock_alliance(2, None);
    alliance_2.executor_corporation_id = None;

    let alliance_repo = AllianceRepository::new(&test.db);
    let result = alliance_repo
        .upsert_many(vec![
            (alliance_id_1, alliance_1, None),
            (alliance_id_2, alliance_2, None),
        ])
        .await?;

    let alliance_1 = result
        .iter()
        .find(|a| a.alliance_id == alliance_id_1)
        .unwrap();
    let alliance_2 = result
        .iter()
        .find(|a| a.alliance_id == alliance_id_2)
        .unwrap();

    assert_eq!(alliance_1.executor_corporation_id, Some(1000));
    assert_eq!(alliance_2.executor_corporation_id, None);

    Ok(())
}
