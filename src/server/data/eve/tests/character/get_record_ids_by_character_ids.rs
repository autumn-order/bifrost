use super::*;

/// Expect Ok with correct mappings when characters exist in database
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

    let character_repo = CharacterRepository::new(&test.state.db);
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

/// Expect Ok with empty Vec when no characters match
#[tokio::test]
async fn returns_empty_for_nonexistent_characters() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    let character_repo = CharacterRepository::new(&test.state.db);
    let character_ids = vec![1, 2, 3];
    let result = character_repo
        .get_record_ids_by_character_ids(&character_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Expect Ok with empty Vec when input is empty
#[tokio::test]
async fn returns_empty_for_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    let character_repo = CharacterRepository::new(&test.state.db);
    let character_ids: Vec<i64> = vec![];
    let result = character_repo
        .get_record_ids_by_character_ids(&character_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Expect Ok with partial results when only some characters exist
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

    let character_repo = CharacterRepository::new(&test.state.db);
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

/// Expect Error when required tables haven't been created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let character_repo = CharacterRepository::new(&test.state.db);
    let character_ids = vec![1, 2, 3];
    let result = character_repo
        .get_record_ids_by_character_ids(&character_ids)
        .await;

    assert!(result.is_err());

    Ok(())
}
