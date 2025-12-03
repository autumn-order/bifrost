//! Tests for CorporationRepository::get_record_ids_by_corporation_ids method.
//!
//! This module verifies the corporation record ID lookup behavior, including successful
//! mappings for existing corporations, handling of nonexistent or mixed inputs, and
//! error handling for missing database tables.

use super::*;

/// Tests retrieving record IDs for existing corporations.
///
/// Verifies that the corporation repository correctly maps corporation IDs to their
/// corresponding database record IDs when all requested corporations exist.
///
/// Expected: Ok with Vec of (record_id, corporation_id) tuples
#[tokio::test]
async fn returns_record_ids_for_existing_corporations() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let corporation_1 = test.eve().insert_mock_corporation(1, None, None).await?;
    let corporation_2 = test.eve().insert_mock_corporation(2, None, None).await?;
    let corporation_3 = test.eve().insert_mock_corporation(3, None, None).await?;

    let corporation_repo = CorporationRepository::new(&test.db);
    let corporation_ids = vec![
        corporation_1.corporation_id,
        corporation_2.corporation_id,
        corporation_3.corporation_id,
    ];
    let result = corporation_repo
        .get_record_ids_by_corporation_ids(&corporation_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 3);

    // Verify the mappings are correct
    let mut found_ids = std::collections::HashSet::new();
    for (record_id, corporation_id) in record_ids {
        match corporation_id {
            _ if corporation_id == corporation_1.corporation_id => {
                assert_eq!(record_id, corporation_1.id);
            }
            _ if corporation_id == corporation_2.corporation_id => {
                assert_eq!(record_id, corporation_2.id);
            }
            _ if corporation_id == corporation_3.corporation_id => {
                assert_eq!(record_id, corporation_3.id);
            }
            _ => panic!("Unexpected corporation_id: {}", corporation_id),
        }
        found_ids.insert(corporation_id);
    }
    assert_eq!(found_ids.len(), 3);

    Ok(())
}

/// Tests retrieving record IDs for nonexistent corporations.
///
/// Verifies that the corporation repository returns an empty list when attempting
/// to retrieve record IDs for corporation IDs that do not exist in the database.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_nonexistent_corporations() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    let corporation_repo = CorporationRepository::new(&test.db);
    let corporation_ids = vec![1, 2, 3];
    let result = corporation_repo
        .get_record_ids_by_corporation_ids(&corporation_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Tests retrieving record IDs with empty input.
///
/// Verifies that the corporation repository handles empty input lists gracefully
/// by returning an empty result without errors.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    let corporation_repo = CorporationRepository::new(&test.db);
    let corporation_ids: Vec<i64> = vec![];
    let result = corporation_repo
        .get_record_ids_by_corporation_ids(&corporation_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Tests retrieving record IDs with mixed input.
///
/// Verifies that the corporation repository returns partial results when only some
/// of the requested corporation IDs exist, excluding nonexistent IDs from the output.
///
/// Expected: Ok with Vec containing only existing corporation mappings
#[tokio::test]
async fn returns_partial_results_for_mixed_input() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let corporation_1 = test.eve().insert_mock_corporation(1, None, None).await?;
    let corporation_3 = test.eve().insert_mock_corporation(3, None, None).await?;

    let corporation_repo = CorporationRepository::new(&test.db);
    let corporation_ids = vec![
        corporation_1.corporation_id,
        999, // Non-existent
        corporation_3.corporation_id,
        888, // Non-existent
    ];
    let result = corporation_repo
        .get_record_ids_by_corporation_ids(&corporation_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 2);

    // Verify only existing corporations are returned
    for (record_id, corporation_id) in record_ids {
        assert!(
            corporation_id == corporation_1.corporation_id
                || corporation_id == corporation_3.corporation_id
        );
        if corporation_id == corporation_1.corporation_id {
            assert_eq!(record_id, corporation_1.id);
        } else if corporation_id == corporation_3.corporation_id {
            assert_eq!(record_id, corporation_3.id);
        }
    }

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the corporation repository returns an error when attempting to
/// retrieve record IDs without the required database tables being created.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let corporation_repo = CorporationRepository::new(&test.db);
    let corporation_ids = vec![1, 2, 3];
    let result = corporation_repo
        .get_record_ids_by_corporation_ids(&corporation_ids)
        .await;

    assert!(result.is_err());

    Ok(())
}
