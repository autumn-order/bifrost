//! Tests for FactionRepository::get_record_ids_by_faction_ids method.
//!
//! This module verifies the faction record ID lookup behavior, including
//! finding existing factions and handling various edge cases.

use super::*;

/// Tests retrieving record IDs for existing factions.
///
/// Verifies that the faction repository correctly maps faction IDs to their
/// corresponding database record IDs when all requested factions exist.
///
/// Expected: Ok with Vec of (record_id, faction_id) tuples
#[tokio::test]
async fn returns_record_ids_for_existing_factions() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;
    let faction_1 = test.eve().insert_mock_faction(1).await?;
    let faction_2 = test.eve().insert_mock_faction(2).await?;
    let faction_3 = test.eve().insert_mock_faction(3).await?;

    let repo = FactionRepository::new(&test.db);
    let faction_ids = vec![
        faction_1.faction_id,
        faction_2.faction_id,
        faction_3.faction_id,
    ];
    let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 3);

    // Verify the mappings are correct
    let mut found_ids = std::collections::HashSet::new();
    for (record_id, faction_id) in record_ids {
        match faction_id {
            _ if faction_id == faction_1.faction_id => {
                assert_eq!(record_id, faction_1.id);
            }
            _ if faction_id == faction_2.faction_id => {
                assert_eq!(record_id, faction_2.id);
            }
            _ if faction_id == faction_3.faction_id => {
                assert_eq!(record_id, faction_3.id);
            }
            _ => panic!("Unexpected faction_id: {}", faction_id),
        }
        found_ids.insert(faction_id);
    }
    assert_eq!(found_ids.len(), 3);

    Ok(())
}

/// Tests retrieving record IDs for nonexistent factions.
///
/// Verifies that the faction repository returns an empty list when attempting
/// to retrieve record IDs for faction IDs that do not exist in the database.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_nonexistent_factions() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    let repo = FactionRepository::new(&test.db);
    let faction_ids = vec![1, 2, 3];
    let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Tests retrieving record IDs with empty input.
///
/// Verifies that the faction repository handles empty input lists gracefully
/// by returning an empty result without errors.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    let repo = FactionRepository::new(&test.db);
    let faction_ids: Vec<i64> = vec![];
    let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Tests retrieving record IDs with mixed input.
///
/// Verifies that the faction repository returns partial results when only some
/// of the requested faction IDs exist, excluding nonexistent IDs from the output.
///
/// Expected: Ok with Vec containing only existing faction mappings
#[tokio::test]
async fn returns_partial_results_for_mixed_input() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;
    let faction_1 = test.eve().insert_mock_faction(1).await?;
    let faction_3 = test.eve().insert_mock_faction(3).await?;

    let repo = FactionRepository::new(&test.db);
    let faction_ids = vec![
        faction_1.faction_id,
        999, // Non-existent
        faction_3.faction_id,
        888, // Non-existent
    ];
    let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 2);

    // Verify only existing factions are returned
    for (record_id, faction_id) in record_ids {
        assert!(faction_id == faction_1.faction_id || faction_id == faction_3.faction_id);
        if faction_id == faction_1.faction_id {
            assert_eq!(record_id, faction_1.id);
        } else if faction_id == faction_3.faction_id {
            assert_eq!(record_id, faction_3.id);
        }
    }

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the faction repository returns an error when attempting to
/// retrieve record IDs without the required database tables being created.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let repo = FactionRepository::new(&test.db);
    let faction_ids = vec![1, 2, 3];
    let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

    assert!(result.is_err());

    Ok(())
}

/// Tests retrieving a single faction record ID.
///
/// Verifies that the method works correctly when querying for a single faction.
///
/// Expected: Ok with single tuple in Vec
#[tokio::test]
async fn returns_single_faction_record_id() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;
    let faction = test.eve().insert_mock_faction(1).await?;

    let repo = FactionRepository::new(&test.db);
    let result = repo
        .get_record_ids_by_faction_ids(&[faction.faction_id])
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 1);
    assert_eq!(record_ids[0].0, faction.id);
    assert_eq!(record_ids[0].1, faction.faction_id);

    Ok(())
}

/// Tests retrieving record IDs with duplicate faction IDs in input.
///
/// Verifies that the repository handles duplicate IDs in the input correctly,
/// returning a single entry per unique faction.
///
/// Expected: Ok with unique faction mappings
#[tokio::test]
async fn handles_duplicate_input_ids() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;
    let faction = test.eve().insert_mock_faction(1).await?;

    let repo = FactionRepository::new(&test.db);
    // Request same faction ID multiple times
    let faction_ids = vec![faction.faction_id, faction.faction_id, faction.faction_id];
    let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    // Should still return single result (database handles duplicates)
    assert_eq!(record_ids.len(), 1);
    assert_eq!(record_ids[0].1, faction.faction_id);

    Ok(())
}
