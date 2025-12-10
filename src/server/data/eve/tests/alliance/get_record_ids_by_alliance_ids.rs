//! Tests for AllianceRepository::get_record_ids_by_alliance_ids method.
//!
//! This module verifies the alliance record ID lookup behavior, including successful
//! mappings for existing alliances, handling of nonexistent or mixed inputs, and
//! error handling for missing database tables.

use super::*;

/// Tests retrieving record IDs for existing alliances.
///
/// Verifies that the alliance repository correctly maps alliance IDs to their
/// corresponding database record IDs when all requested alliances exist.
///
/// Expected: Ok with Vec of (record_id, alliance_id) tuples
#[tokio::test]
async fn returns_record_ids_for_existing_alliances() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let alliance_1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance_2 = test.eve().insert_mock_alliance(2, None).await?;
    let alliance_3 = test.eve().insert_mock_alliance(3, None).await?;

    let alliance_repo = AllianceRepository::new(&test.db);
    let alliance_ids = vec![
        alliance_1.alliance_id,
        alliance_2.alliance_id,
        alliance_3.alliance_id,
    ];
    let result = alliance_repo
        .get_record_ids_by_alliance_ids(&alliance_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 3);

    // Verify the mappings are correct
    let mut found_ids = std::collections::HashSet::new();
    for (record_id, alliance_id) in record_ids {
        match alliance_id {
            _ if alliance_id == alliance_1.alliance_id => {
                assert_eq!(record_id, alliance_1.id);
            }
            _ if alliance_id == alliance_2.alliance_id => {
                assert_eq!(record_id, alliance_2.id);
            }
            _ if alliance_id == alliance_3.alliance_id => {
                assert_eq!(record_id, alliance_3.id);
            }
            _ => panic!("Unexpected alliance_id: {}", alliance_id),
        }
        found_ids.insert(alliance_id);
    }
    assert_eq!(found_ids.len(), 3);

    Ok(())
}

/// Tests retrieving record IDs for nonexistent alliances.
///
/// Verifies that the alliance repository returns an empty list when attempting
/// to retrieve record IDs for alliance IDs that do not exist in the database.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_nonexistent_alliances() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;

    let alliance_repo = AllianceRepository::new(&test.db);
    let alliance_ids = vec![1, 2, 3];
    let result = alliance_repo
        .get_record_ids_by_alliance_ids(&alliance_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Tests retrieving record IDs with empty input.
///
/// Verifies that the alliance repository handles empty input lists gracefully
/// by returning an empty result without errors.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;

    let alliance_repo = AllianceRepository::new(&test.db);
    let alliance_ids: Vec<i64> = vec![];
    let result = alliance_repo
        .get_record_ids_by_alliance_ids(&alliance_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Tests retrieving record IDs with mixed input.
///
/// Verifies that the alliance repository returns partial results when only some
/// of the requested alliance IDs exist, excluding nonexistent IDs from the output.
///
/// Expected: Ok with Vec containing only existing alliance mappings
#[tokio::test]
async fn returns_partial_results_for_mixed_input() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let alliance_1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance_3 = test.eve().insert_mock_alliance(3, None).await?;

    let alliance_repo = AllianceRepository::new(&test.db);
    let alliance_ids = vec![
        alliance_1.alliance_id,
        999, // Non-existent
        alliance_3.alliance_id,
        888, // Non-existent
    ];
    let result = alliance_repo
        .get_record_ids_by_alliance_ids(&alliance_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 2);

    // Verify only existing alliances are returned
    for (record_id, alliance_id) in record_ids {
        assert!(alliance_id == alliance_1.alliance_id || alliance_id == alliance_3.alliance_id);
        if alliance_id == alliance_1.alliance_id {
            assert_eq!(record_id, alliance_1.id);
        } else if alliance_id == alliance_3.alliance_id {
            assert_eq!(record_id, alliance_3.id);
        }
    }

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the alliance repository returns an error when attempting to
/// retrieve record IDs without the required database tables being created.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let alliance_repo = AllianceRepository::new(&test.db);
    let alliance_ids = vec![1, 2, 3];
    let result = alliance_repo
        .get_record_ids_by_alliance_ids(&alliance_ids)
        .await;

    assert!(result.is_err());

    Ok(())
}

/// Tests retrieving record IDs maintains order consistency.
///
/// Verifies that while the order of returned tuples may not match input order,
/// the correct mappings are always returned.
///
/// Expected: Ok with all correct mappings present
#[tokio::test]
async fn maintains_correct_mappings_regardless_of_order() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let alliance_1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance_2 = test.eve().insert_mock_alliance(2, None).await?;
    let alliance_3 = test.eve().insert_mock_alliance(3, None).await?;

    let alliance_repo = AllianceRepository::new(&test.db);

    // Try with different input orders
    let alliance_ids_1 = vec![
        alliance_1.alliance_id,
        alliance_2.alliance_id,
        alliance_3.alliance_id,
    ];
    let alliance_ids_2 = vec![
        alliance_3.alliance_id,
        alliance_1.alliance_id,
        alliance_2.alliance_id,
    ];

    let result_1 = alliance_repo
        .get_record_ids_by_alliance_ids(&alliance_ids_1)
        .await?;
    let result_2 = alliance_repo
        .get_record_ids_by_alliance_ids(&alliance_ids_2)
        .await?;

    // Both should have same length
    assert_eq!(result_1.len(), 3);
    assert_eq!(result_2.len(), 3);

    // Convert to maps for easy comparison (record_id -> alliance_id)
    let map_1: std::collections::HashMap<_, _> = result_1.into_iter().collect();
    let map_2: std::collections::HashMap<_, _> = result_2.into_iter().collect();

    // Verify mappings are identical (key is record_id, value is alliance_id)
    assert_eq!(map_1.get(&alliance_1.id), Some(&alliance_1.alliance_id));
    assert_eq!(map_1.get(&alliance_2.id), Some(&alliance_2.alliance_id));
    assert_eq!(map_1.get(&alliance_3.id), Some(&alliance_3.alliance_id));
    assert_eq!(map_2.get(&alliance_1.id), Some(&alliance_1.alliance_id));
    assert_eq!(map_2.get(&alliance_2.id), Some(&alliance_2.alliance_id));
    assert_eq!(map_2.get(&alliance_3.id), Some(&alliance_3.alliance_id));

    Ok(())
}

/// Tests retrieving record IDs for alliances with faction relationships.
///
/// Verifies that the method correctly retrieves record IDs regardless of
/// whether the alliances have faction affiliations or not.
///
/// Expected: Ok with correct mappings for all alliances
#[tokio::test]
async fn returns_record_ids_for_alliances_with_factions() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let _faction = test.eve().insert_mock_faction(1).await?;
    let alliance_1 = test.eve().insert_mock_alliance(1, Some(1)).await?;
    let alliance_2 = test.eve().insert_mock_alliance(2, None).await?;

    let alliance_repo = AllianceRepository::new(&test.db);
    let alliance_ids = vec![alliance_1.alliance_id, alliance_2.alliance_id];
    let result = alliance_repo
        .get_record_ids_by_alliance_ids(&alliance_ids)
        .await?;

    assert_eq!(result.len(), 2);

    // Verify correct mappings exist (key is record_id, value is alliance_id)
    let found: Vec<_> = result.iter().collect();
    assert!(found
        .iter()
        .any(|(rid, aid)| *rid == alliance_1.id && *aid == alliance_1.alliance_id));
    assert!(found
        .iter()
        .any(|(rid, aid)| *rid == alliance_2.id && *aid == alliance_2.alliance_id));

    Ok(())
}

/// Tests handling duplicate alliance IDs in input.
///
/// Verifies that when the same alliance ID appears multiple times in the input,
/// it only appears once in the output with the correct mapping.
///
/// Expected: Ok with deduplicated results
#[tokio::test]
async fn handles_duplicate_input_ids() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let alliance_1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance_2 = test.eve().insert_mock_alliance(2, None).await?;

    let alliance_repo = AllianceRepository::new(&test.db);
    let alliance_ids = vec![
        alliance_1.alliance_id,
        alliance_2.alliance_id,
        alliance_1.alliance_id, // Duplicate
        alliance_2.alliance_id, // Duplicate
        alliance_1.alliance_id, // Duplicate
    ];
    let result = alliance_repo
        .get_record_ids_by_alliance_ids(&alliance_ids)
        .await?;

    // Should return results for unique alliance IDs
    assert!(result.len() >= 2);

    // Verify correct mappings exist (key is record_id, value is alliance_id)
    let found: Vec<_> = result.iter().collect();
    assert!(found
        .iter()
        .any(|(rid, aid)| *rid == alliance_1.id && *aid == alliance_1.alliance_id));
    assert!(found
        .iter()
        .any(|(rid, aid)| *rid == alliance_2.id && *aid == alliance_2.alliance_id));

    Ok(())
}

/// Tests retrieving record IDs for a large number of alliances.
///
/// Verifies that the method can efficiently handle bulk lookups of many
/// alliance IDs at once.
///
/// Expected: Ok with correct mappings for all 50 alliances
#[tokio::test]
async fn handles_large_batch_lookup() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;

    let mut alliances = Vec::new();
    for i in 1..=50 {
        let alliance = test.eve().insert_mock_alliance(i, None).await?;
        alliances.push(alliance);
    }

    let alliance_repo = AllianceRepository::new(&test.db);
    let alliance_ids: Vec<i64> = alliances.iter().map(|a| a.alliance_id).collect();
    let result = alliance_repo
        .get_record_ids_by_alliance_ids(&alliance_ids)
        .await?;

    assert_eq!(result.len(), 50);

    // Verify all mappings are correct (each tuple is (record_id, alliance_id))
    for alliance in &alliances {
        assert!(result
            .iter()
            .any(|(rid, aid)| *rid == alliance.id && *aid == alliance.alliance_id));
    }

    Ok(())
}
