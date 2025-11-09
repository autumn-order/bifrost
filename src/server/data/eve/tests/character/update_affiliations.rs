use super::*;

/// Should successfully update a single character's corporation and faction affiliation
#[tokio::test]
async fn updates_single_character_affiliation() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Create factions and corporations
    let faction1 = test.eve().insert_mock_faction(1).await?;
    let faction2 = test.eve().insert_mock_faction(2).await?;
    let corp1 = test
        .eve()
        .insert_mock_corporation(100, None, Some(faction1.faction_id))
        .await?;
    let corp2 = test
        .eve()
        .insert_mock_corporation(200, None, Some(faction2.faction_id))
        .await?;

    // Create a character initially affiliated with corp1 and faction1
    let (character_id, character) =
        test.eve()
            .with_mock_character(1, corp1.corporation_id, None, None);
    let character_repo = CharacterRepository::new(&test.state.db);
    let char = character_repo
        .create(character_id, character, corp1.id, Some(faction1.id))
        .await?;

    // Update character to be affiliated with corp2 and faction2
    let result = character_repo
        .update_affiliations(vec![(char.id, corp2.id, Some(faction2.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the update
    let updated = character_repo
        .get_by_character_id(char.character_id)
        .await?
        .expect("Character should exist");

    assert_eq!(updated.corporation_id, corp2.id);
    assert_eq!(updated.faction_id, Some(faction2.id));

    Ok(())
}

/// Should successfully update multiple characters in a single call
#[tokio::test]
async fn updates_multiple_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Create factions and corporations
    let faction1 = test.eve().insert_mock_faction(1).await?;
    let faction2 = test.eve().insert_mock_faction(2).await?;
    let faction3 = test.eve().insert_mock_faction(3).await?;
    let corp1 = test.eve().insert_mock_corporation(100, None, None).await?;
    let corp2 = test.eve().insert_mock_corporation(200, None, None).await?;
    let corp3 = test.eve().insert_mock_corporation(300, None, None).await?;

    // Create characters
    let char1 = test
        .eve()
        .insert_mock_character(1, corp1.corporation_id, None, None)
        .await?;

    let char2 = test
        .eve()
        .insert_mock_character(2, corp1.corporation_id, None, None)
        .await?;

    let char3 = test
        .eve()
        .insert_mock_character(3, corp1.corporation_id, None, None)
        .await?;

    // Update multiple characters
    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo
        .update_affiliations(vec![
            (char1.id, corp1.id, Some(faction1.id)),
            (char2.id, corp2.id, Some(faction2.id)),
            (char3.id, corp3.id, Some(faction3.id)),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify all updates
    let updated1 = character_repo
        .get_by_character_id(char1.character_id)
        .await?
        .expect("Character 1 should exist");
    let updated2 = character_repo
        .get_by_character_id(char2.character_id)
        .await?
        .expect("Character 2 should exist");
    let updated3 = character_repo
        .get_by_character_id(char3.character_id)
        .await?
        .expect("Character 3 should exist");

    assert_eq!(updated1.corporation_id, corp1.id);
    assert_eq!(updated1.faction_id, Some(faction1.id));
    assert_eq!(updated2.corporation_id, corp2.id);
    assert_eq!(updated2.faction_id, Some(faction2.id));
    assert_eq!(updated3.corporation_id, corp3.id);
    assert_eq!(updated3.faction_id, Some(faction3.id));

    Ok(())
}

/// Should successfully remove faction affiliation by setting to None
#[tokio::test]
async fn removes_faction_affiliation() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Create faction and corporation
    let faction = test.eve().insert_mock_faction(1).await?;
    let corp = test.eve().insert_mock_corporation(100, None, None).await?;

    // Create a character with a faction
    let (character_id, character) =
        test.eve()
            .with_mock_character(1, corp.corporation_id, None, None);
    let character_repo = CharacterRepository::new(&test.state.db);
    let char = character_repo
        .create(character_id, character, corp.id, Some(faction.id))
        .await?;

    // Remove faction affiliation
    let result = character_repo
        .update_affiliations(vec![(char.id, corp.id, None)])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the faction was removed
    let updated = character_repo
        .get_by_character_id(char.character_id)
        .await?
        .expect("Character should exist");

    assert_eq!(updated.faction_id, None);

    Ok(())
}

/// Should handle batching for large numbers of characters (>100)
#[tokio::test]
async fn handles_large_batch_updates() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Create a corporation and faction
    let corp = test.eve().insert_mock_corporation(100, None, None).await?;
    let faction = test.eve().insert_mock_faction(1).await?;

    // Create 250 characters (more than 2x BATCH_SIZE)
    let mut characters = Vec::new();
    for i in 0..250 {
        let char = test
            .eve()
            .insert_mock_character(1000 + i, corp.corporation_id, None, None)
            .await?;

        characters.push((char.id, corp.id, Some(faction.id)));
    }

    // Update all characters
    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo.update_affiliations(characters).await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify a sample of updates
    let updated_first = character_repo
        .get_by_character_id(1000)
        .await?
        .expect("First character should exist");
    let updated_middle = character_repo
        .get_by_character_id(1125)
        .await?
        .expect("Middle character should exist");
    let updated_last = character_repo
        .get_by_character_id(1249)
        .await?
        .expect("Last character should exist");

    assert_eq!(updated_first.faction_id, Some(faction.id));
    assert_eq!(updated_middle.faction_id, Some(faction.id));
    assert_eq!(updated_last.faction_id, Some(faction.id));

    Ok(())
}

/// Should handle empty input gracefully
#[tokio::test]
async fn handles_empty_input() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo.update_affiliations(vec![]).await;

    assert!(result.is_ok(), "Should handle empty input gracefully");

    Ok(())
}

/// Should update UpdatedAt timestamp when updating affiliations
#[tokio::test]
async fn updates_timestamp() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Create corporation and faction
    let corp = test.eve().insert_mock_corporation(100, None, None).await?;
    let faction = test.eve().insert_mock_faction(1).await?;

    // Create a character
    let (character_id, character) =
        test.eve()
            .with_mock_character(1, corp.corporation_id, None, None);
    let character_repo = CharacterRepository::new(&test.state.db);
    let char = character_repo
        .create(character_id, character, corp.id, None)
        .await?;

    let original_updated_at = char.info_updated_at;

    // Wait a moment to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Update the character
    let result = character_repo
        .update_affiliations(vec![(char.id, corp.id, Some(faction.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the timestamp was updated
    let updated = character_repo
        .get_by_character_id(char.character_id)
        .await?
        .expect("Character should exist");

    assert!(
        updated.info_updated_at >= original_updated_at,
        "UpdatedAt should be equal to or newer than original. Original: {:?}, Updated: {:?}",
        original_updated_at,
        updated.info_updated_at
    );

    Ok(())
}

/// Should not affect characters not in the update list
#[tokio::test]
async fn does_not_affect_other_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Create factions and corporations
    let faction1 = test.eve().insert_mock_faction(1).await?;
    let faction2 = test.eve().insert_mock_faction(2).await?;
    let corp1 = test.eve().insert_mock_corporation(100, None, None).await?;
    let corp2 = test.eve().insert_mock_corporation(200, None, None).await?;

    // Create characters
    let char1 = test
        .eve()
        .insert_mock_character(1, corp1.corporation_id, None, Some(faction1.faction_id))
        .await?;
    let char2 = test
        .eve()
        .insert_mock_character(2, corp1.corporation_id, None, Some(faction1.faction_id))
        .await?;

    // Update only char1
    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo
        .update_affiliations(vec![(char1.id, corp2.id, Some(faction2.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify char1 was updated
    let updated1 = character_repo
        .get_by_character_id(char1.character_id)
        .await?
        .expect("Character 1 should exist");
    assert_eq!(updated1.corporation_id, corp2.id);
    assert_eq!(updated1.faction_id, Some(faction2.id));

    // Verify char2 was NOT updated
    let updated2 = character_repo
        .get_by_character_id(char2.character_id)
        .await?
        .expect("Character 2 should exist");
    assert_eq!(
        updated2.corporation_id, corp1.id,
        "Character 2 should still have original corporation"
    );
    assert_eq!(
        updated2.faction_id,
        Some(faction1.id),
        "Character 2 should still have original faction"
    );

    Ok(())
}

/// Should handle mix of Some and None faction IDs in same batch
#[tokio::test]
async fn handles_mixed_faction_assignments() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Create faction and corporation
    let faction = test.eve().insert_mock_faction(1).await?;
    let corp = test.eve().insert_mock_corporation(100, None, None).await?;

    // Create characters
    let char1 = test
        .eve()
        .insert_mock_character(1, corp.corporation_id, None, None)
        .await?;
    let char2 = test
        .eve()
        .insert_mock_character(2, corp.corporation_id, None, None)
        .await?;
    let char3 = test
        .eve()
        .insert_mock_character(3, corp.corporation_id, None, None)
        .await?;

    // Update with mixed faction IDs
    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo
        .update_affiliations(vec![
            (char1.id, corp.id, Some(faction.id)),
            (char2.id, corp.id, None),
            (char3.id, corp.id, Some(faction.id)),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify updates
    let updated1 = character_repo
        .get_by_character_id(char1.character_id)
        .await?
        .expect("Character 1 should exist");
    let updated2 = character_repo
        .get_by_character_id(char2.character_id)
        .await?
        .expect("Character 2 should exist");
    let updated3 = character_repo
        .get_by_character_id(char3.character_id)
        .await?
        .expect("Character 3 should exist");

    assert_eq!(updated1.faction_id, Some(faction.id));
    assert_eq!(updated2.faction_id, None);
    assert_eq!(updated3.faction_id, Some(faction.id));

    Ok(())
}
