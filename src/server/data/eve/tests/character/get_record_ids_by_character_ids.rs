//! Tests for CharacterRepository::get_record_ids_by_character_ids method.
//!
//! This module verifies the character record ID lookup behavior, including successful
//! mappings for existing characters, handling of nonexistent or mixed inputs, and
//! error handling for missing database tables.

use super::*;

/// Tests retrieving record IDs for existing characters.
///
/// Verifies that the character repository correctly maps character IDs to their
/// corresponding database record IDs when all requested characters exist.
///
/// Expected: Ok with Vec of (record_id, character_id) tuples
#[tokio::test]
async fn returns_record_ids_for_existing_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;
    let character_1 = test.eve().insert_mock_character(1, 1, None, None).await?;
    let character_2 = test.eve().insert_mock_character(2, 1, None, None).await?;
    let character_3 = test.eve().insert_mock_character(3, 1, None, None).await?;

    let character_repo = CharacterRepository::new(&test.db);
    let character_ids = vec![
        character_1.character_id,
        character_2.character_id,
        character_3.character_id,
    ];
    let result = character_repo
        .get_record_ids_by_character_ids(&character_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 3);

    // Verify the mappings are correct
    let mut found_ids = std::collections::HashSet::new();
    for (record_id, character_id) in record_ids {
        match character_id {
            _ if character_id == character_1.character_id => {
                assert_eq!(record_id, character_1.id);
            }
            _ if character_id == character_2.character_id => {
                assert_eq!(record_id, character_2.id);
            }
            _ if character_id == character_3.character_id => {
                assert_eq!(record_id, character_3.id);
            }
            _ => panic!("Unexpected character_id: {}", character_id),
        }
        found_ids.insert(character_id);
    }
    assert_eq!(found_ids.len(), 3);

    Ok(())
}

/// Tests retrieving record IDs for nonexistent characters.
///
/// Verifies that the character repository returns an empty list when attempting
/// to retrieve record IDs for character IDs that do not exist in the database.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_nonexistent_characters() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    let character_repo = CharacterRepository::new(&test.db);
    let character_ids = vec![1, 2, 3];
    let result = character_repo
        .get_record_ids_by_character_ids(&character_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Tests retrieving record IDs with empty input.
///
/// Verifies that the character repository handles empty input lists gracefully
/// by returning an empty result without errors.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    let character_repo = CharacterRepository::new(&test.db);
    let character_ids: Vec<i64> = vec![];
    let result = character_repo
        .get_record_ids_by_character_ids(&character_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Tests retrieving record IDs with mixed input.
///
/// Verifies that the character repository returns partial results when only some
/// of the requested character IDs exist, excluding nonexistent IDs from the output.
///
/// Expected: Ok with Vec containing only existing character mappings
#[tokio::test]
async fn returns_partial_results_for_mixed_input() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;
    let character_1 = test.eve().insert_mock_character(1, 1, None, None).await?;
    let character_3 = test.eve().insert_mock_character(3, 1, None, None).await?;

    let character_repo = CharacterRepository::new(&test.db);
    let character_ids = vec![
        character_1.character_id,
        999, // Non-existent
        character_3.character_id,
        888, // Non-existent
    ];
    let result = character_repo
        .get_record_ids_by_character_ids(&character_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 2);

    // Verify only existing characters are returned
    for (record_id, character_id) in record_ids {
        assert!(
            character_id == character_1.character_id || character_id == character_3.character_id
        );
        if character_id == character_1.character_id {
            assert_eq!(record_id, character_1.id);
        } else if character_id == character_3.character_id {
            assert_eq!(record_id, character_3.id);
        }
    }

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the character repository returns an error when attempting to
/// retrieve record IDs without the required database tables being created.
///
/// Expected: Err
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let character_repo = CharacterRepository::new(&test.db);
    let character_ids = vec![1, 2, 3];
    let result = character_repo
        .get_record_ids_by_character_ids(&character_ids)
        .await;

    assert!(result.is_err());

    Ok(())
}
