use super::*;

/// Expect Ok when upserting new characters
#[tokio::test]
async fn upserts_new_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id_1, character_1) =
        test.eve()
            .with_mock_character(1, corporation_model.corporation_id, None, None);
    let (character_id_2, character_2) =
        test.eve()
            .with_mock_character(2, corporation_model.corporation_id, None, None);

    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo
        .upsert_many(vec![
            (character_id_1, character_1, corporation_model.id, None),
            (character_id_2, character_2, corporation_model.id, None),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let created_characters = result.unwrap();
    assert_eq!(created_characters.len(), 2);

    Ok(())
}

/// Expect Ok & update when trying to upsert existing characters
#[tokio::test]
async fn updates_existing_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id_1, character_1) =
        test.eve()
            .with_mock_character(1, corporation_model.corporation_id, None, None);
    let (character_id_2, character_2) =
        test.eve()
            .with_mock_character(2, corporation_model.corporation_id, None, None);
    let (character_id_1_update, mut character_1_update) =
        test.eve()
            .with_mock_character(1, corporation_model.corporation_id, None, None);
    let (character_id_2_update, mut character_2_update) =
        test.eve()
            .with_mock_character(2, corporation_model.corporation_id, None, None);

    let character_repo = CharacterRepository::new(&test.state.db);
    let initial = character_repo
        .upsert_many(vec![
            (character_id_1, character_1, corporation_model.id, None),
            (character_id_2, character_2, corporation_model.id, None),
        ])
        .await?;

    let initial_entry_1 = initial
        .iter()
        .find(|c| c.character_id == character_id_1)
        .expect("character 1 not found");
    let initial_entry_2 = initial
        .iter()
        .find(|c| c.character_id == character_id_2)
        .expect("character 2 not found");

    let initial_created_at_1 = initial_entry_1.created_at;
    let initial_updated_at_1 = initial_entry_1.info_updated_at;
    let initial_created_at_2 = initial_entry_2.created_at;
    let initial_updated_at_2 = initial_entry_2.info_updated_at;

    // Modify character data to verify updates
    character_1_update.name = "Updated Character 1".to_string();
    character_2_update.name = "Updated Character 2".to_string();

    let latest = character_repo
        .upsert_many(vec![
            (
                character_id_1_update,
                character_1_update,
                corporation_model.id,
                None,
            ),
            (
                character_id_2_update,
                character_2_update,
                corporation_model.id,
                None,
            ),
        ])
        .await?;

    let latest_entry_1 = latest
        .iter()
        .find(|c| c.character_id == character_id_1_update)
        .expect("character 1 not found");
    let latest_entry_2 = latest
        .iter()
        .find(|c| c.character_id == character_id_2_update)
        .expect("character 2 not found");

    // created_at should not change and updated_at should increase for both characters
    assert_eq!(latest_entry_1.created_at, initial_created_at_1);
    assert!(latest_entry_1.info_updated_at > initial_updated_at_1);
    assert_eq!(latest_entry_1.name, "Updated Character 1");
    assert_eq!(latest_entry_2.created_at, initial_created_at_2);
    assert!(latest_entry_2.info_updated_at > initial_updated_at_2);
    assert_eq!(latest_entry_2.name, "Updated Character 2");

    Ok(())
}

/// Expect Ok when upserting mix of new and existing characters
#[tokio::test]
async fn upserts_mixed_new_and_existing_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id_1, character_1) =
        test.eve()
            .with_mock_character(1, corporation_model.corporation_id, None, None);
    let (character_id_2, character_2) =
        test.eve()
            .with_mock_character(2, corporation_model.corporation_id, None, None);
    let (character_id_3, character_3) =
        test.eve()
            .with_mock_character(3, corporation_model.corporation_id, None, None);
    let (character_id_1_update, mut character_1_update) =
        test.eve()
            .with_mock_character(1, corporation_model.corporation_id, None, None);
    character_1_update.name = "Updated Character 1".to_string();
    let (character_id_2_update, character_2_update) =
        test.eve()
            .with_mock_character(2, corporation_model.corporation_id, None, None);

    let character_repo = CharacterRepository::new(&test.state.db);

    // First, insert characters 1 and 2
    let initial = character_repo
        .upsert_many(vec![
            (character_id_1, character_1, corporation_model.id, None),
            (character_id_2, character_2, corporation_model.id, None),
        ])
        .await?;

    assert_eq!(initial.len(), 2);
    let initial_char_1 = initial
        .iter()
        .find(|c| c.character_id == character_id_1)
        .expect("character 1 not found");
    let initial_created_at = initial_char_1.created_at;

    // Now upsert characters 1 (update), 2 (update), and 3 (new)
    let result = character_repo
        .upsert_many(vec![
            (
                character_id_1_update,
                character_1_update,
                corporation_model.id,
                None,
            ),
            (
                character_id_2_update,
                character_2_update,
                corporation_model.id,
                None,
            ),
            (character_id_3, character_3, corporation_model.id, None),
        ])
        .await?;

    assert_eq!(result.len(), 3);

    let updated_char_1 = result
        .iter()
        .find(|c| c.character_id == character_id_1)
        .expect("character 1 not found");
    let char_3 = result
        .iter()
        .find(|c| c.character_id == character_id_3)
        .expect("character 3 not found");

    // Character 1 should be updated (same created_at, changed name)
    assert_eq!(updated_char_1.created_at, initial_created_at);
    assert_eq!(updated_char_1.name, "Updated Character 1");

    // Character 3 should be newly created
    assert_eq!(char_3.character_id, character_id_3);

    Ok(())
}

/// Expect Ok with empty result when upserting empty vector
#[tokio::test]
async fn handles_empty_input() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo.upsert_many(vec![]).await?;

    assert_eq!(result.len(), 0);

    Ok(())
}

/// Expect Ok when upserting characters with various faction relationships
#[tokio::test]
async fn upserts_with_faction_relationships() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;
    let faction_1 = test.eve().insert_mock_faction(1).await?;
    let faction_2 = test.eve().insert_mock_faction(2).await?;

    let (character_id_1, character_1) =
        test.eve()
            .with_mock_character(1, corporation_model.corporation_id, None, None);
    let (character_id_2, character_2) =
        test.eve()
            .with_mock_character(2, corporation_model.corporation_id, None, None);
    let (character_id_3, character_3) =
        test.eve()
            .with_mock_character(3, corporation_model.corporation_id, None, None);

    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo
        .upsert_many(vec![
            (
                character_id_1,
                character_1,
                corporation_model.id,
                Some(faction_1.id),
            ),
            (
                character_id_2,
                character_2,
                corporation_model.id,
                Some(faction_2.id),
            ),
            (character_id_3, character_3, corporation_model.id, None),
        ])
        .await?;

    assert_eq!(result.len(), 3);

    let char_1 = result
        .iter()
        .find(|c| c.character_id == character_id_1)
        .unwrap();
    let char_2 = result
        .iter()
        .find(|c| c.character_id == character_id_2)
        .unwrap();
    let char_3 = result
        .iter()
        .find(|c| c.character_id == character_id_3)
        .unwrap();

    assert_eq!(char_1.faction_id, Some(faction_1.id));
    assert_eq!(char_2.faction_id, Some(faction_2.id));
    assert_eq!(char_3.faction_id, None);

    Ok(())
}

/// Expect Ok when upserting large batch of characters
#[tokio::test]
async fn handles_large_batch() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

    let mut characters = Vec::new();
    for i in 1..=100 {
        let (character_id, character) =
            test.eve()
                .with_mock_character(i, corporation_model.corporation_id, None, None);
        characters.push((character_id, character, corporation_model.id, None));
    }

    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo.upsert_many(characters).await?;

    assert_eq!(result.len(), 100);

    // Verify all character IDs are present
    for i in 1..=100 {
        assert!(result.iter().any(|c| c.character_id == i));
    }

    Ok(())
}
