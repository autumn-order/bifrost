//! Tests for CharacterRepository::find_by_eve_id method.
//!
//! This module verifies the character lookup behavior by EVE character ID,
//! including finding existing characters and handling non-existent characters.

use super::*;

/// Tests finding an existing character by EVE ID.
///
/// Verifies that the character repository successfully finds and returns a
/// character record when searching by a valid EVE character ID.
///
/// Expected: Ok(Some(character)) with matching character data
#[tokio::test]
async fn finds_existing_character() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;
    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id, character) =
        test.eve()
            .mock_character(1, corporation.corporation_id, None, None);

    let character_repo = CharacterRepository::new(&test.db);
    character_repo
        .upsert_many(vec![(
            character_id,
            character.clone(),
            corporation.id,
            None,
        )])
        .await?;

    let result = character_repo.find_by_eve_id(character_id).await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let found_character = result.unwrap();
    assert!(found_character.is_some());

    let found = found_character.unwrap();
    assert_eq!(found.character_id, character_id);
    assert_eq!(found.name, character.name);
    assert_eq!(found.corporation_id, corporation.id);
    assert_eq!(found.bloodline_id, character.bloodline_id);
    assert_eq!(found.race_id, character.race_id);

    Ok(())
}

/// Tests finding a non-existent character by EVE ID.
///
/// Verifies that the character repository returns None when searching for
/// a character ID that doesn't exist in the database.
///
/// Expected: Ok(None)
#[tokio::test]
async fn returns_none_for_nonexistent_character() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    let character_repo = CharacterRepository::new(&test.db);
    let result = character_repo.find_by_eve_id(999999999).await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let found_character = result.unwrap();
    assert!(found_character.is_none());

    Ok(())
}

/// Tests finding characters with corporation and faction affiliations.
///
/// Verifies that the character repository correctly retrieves characters
/// with their corporation and faction relationships intact.
///
/// Expected: Ok(Some(character)) with correct corporation_id and faction_id
#[tokio::test]
async fn finds_character_with_affiliations() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;
    let faction_id = 500_001;
    let faction = test.eve().insert_mock_faction(faction_id).await?;
    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id, character) =
        test.eve()
            .mock_character(1, corporation.corporation_id, None, Some(faction_id));

    let character_repo = CharacterRepository::new(&test.db);
    character_repo
        .upsert_many(vec![(
            character_id,
            character,
            corporation.id,
            Some(faction.id),
        )])
        .await?;

    let result = character_repo.find_by_eve_id(character_id).await?;

    assert!(result.is_some());
    let found = result.unwrap();
    assert_eq!(found.character_id, character_id);
    assert_eq!(found.corporation_id, corporation.id);
    assert_eq!(found.faction_id, Some(faction.id));

    Ok(())
}

/// Tests finding multiple different characters.
///
/// Verifies that the character repository can correctly find different
/// characters when multiple characters exist in the database.
///
/// Expected: Ok(Some(character)) for each searched character ID
#[tokio::test]
async fn finds_correct_character_among_multiple() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;
    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id_1, character_1) =
        test.eve()
            .mock_character(1, corporation.corporation_id, None, None);
    let (character_id_2, character_2) =
        test.eve()
            .mock_character(2, corporation.corporation_id, None, None);
    let (character_id_3, character_3) =
        test.eve()
            .mock_character(3, corporation.corporation_id, None, None);

    let character_repo = CharacterRepository::new(&test.db);
    character_repo
        .upsert_many(vec![
            (character_id_1, character_1.clone(), corporation.id, None),
            (character_id_2, character_2.clone(), corporation.id, None),
            (character_id_3, character_3.clone(), corporation.id, None),
        ])
        .await?;

    // Find each character and verify correct data is returned
    let found_1 = character_repo
        .find_by_eve_id(character_id_1)
        .await?
        .unwrap();
    let found_2 = character_repo
        .find_by_eve_id(character_id_2)
        .await?
        .unwrap();
    let found_3 = character_repo
        .find_by_eve_id(character_id_3)
        .await?
        .unwrap();

    assert_eq!(found_1.character_id, character_id_1);
    assert_eq!(found_1.name, character_1.name);
    assert_eq!(found_2.character_id, character_id_2);
    assert_eq!(found_2.name, character_2.name);
    assert_eq!(found_3.character_id, character_id_3);
    assert_eq!(found_3.name, character_3.name);

    Ok(())
}
