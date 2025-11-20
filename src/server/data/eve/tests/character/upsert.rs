use super::*;

/// Expect Ok when upserting a new character with faction
#[tokio::test]
async fn creates_new_character_with_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let faction_model = test.eve().insert_mock_faction(1).await?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id, character) =
        test.eve()
            .with_mock_character(1, corporation_model.corporation_id, None, None);

    let character_repo = CharacterRepository::new(test.state.db.clone());
    let result = character_repo
        .upsert(
            character_id,
            character,
            corporation_model.id,
            Some(faction_model.id),
        )
        .await;

    assert!(result.is_ok());
    let created = result.unwrap();
    let (character_id, character) =
        test.eve()
            .with_mock_character(1, corporation_model.corporation_id, None, None);
    assert_eq!(created.character_id, character_id);
    assert_eq!(created.name, character.name);
    assert_eq!(created.faction_id, Some(faction_model.id));

    Ok(())
}

/// Expect Ok when upserting a new character without faction
#[tokio::test]
async fn creates_new_character_without_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id, character) =
        test.eve()
            .with_mock_character(1, corporation_model.corporation_id, None, None);

    let character_repo = CharacterRepository::new(test.state.db.clone());
    let result = character_repo
        .upsert(character_id, character, corporation_model.id, None)
        .await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.faction_id, None);

    Ok(())
}

/// Expect Ok when upserting an existing character and verify it updates
#[tokio::test]
async fn updates_existing_character() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

    // Create updated character data with different values
    let corporation_model = test.eve().insert_mock_corporation(2, None, None).await?;
    let (character_id, mut updated_character) =
        test.eve()
            .with_mock_character(1, corporation_model.corporation_id, None, None);
    updated_character.name = "Updated Character Name".to_string();
    updated_character.description = Some("Updated description".to_string());
    updated_character.security_status = Some(5.0);

    let character_repo = CharacterRepository::new(test.state.db.clone());
    let result = character_repo
        .upsert(
            character_id,
            updated_character,
            character_model.corporation_id,
            None,
        )
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    // Verify the ID remains the same (it's an update, not a new insert)
    assert_eq!(upserted.id, character_model.id);
    assert_eq!(upserted.character_id, character_model.character_id);
    assert_eq!(upserted.name, "Updated Character Name");
    assert_eq!(
        upserted.description,
        Some("Updated description".to_string())
    );
    assert_eq!(upserted.security_status, Some(5.0));

    Ok(())
}

/// Expect Ok when upserting an existing character with a new corporation ID
#[tokio::test]
async fn updates_character_corporation_relationship() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let corporation_model1 = test.eve().insert_mock_corporation(1, None, None).await?;
    let corporation_model2 = test.eve().insert_mock_corporation(2, None, None).await?;
    let character_model = test
        .eve()
        .insert_mock_character(1, corporation_model1.corporation_id, None, None)
        .await?;

    // Update character with new corporation
    let (character_id, character) =
        test.eve()
            .with_mock_character(1, corporation_model2.corporation_id, None, None);

    let character_repo = CharacterRepository::new(test.state.db.clone());
    let result = character_repo
        .upsert(character_id, character, corporation_model2.id, None)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, character_model.id);
    assert_eq!(upserted.corporation_id, corporation_model2.id);
    assert_ne!(upserted.corporation_id, corporation_model1.id);

    Ok(())
}

/// Expect Ok when upserting an existing character with a new faction ID
#[tokio::test]
async fn updates_character_faction_relationship() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let faction_model1 = test.eve().insert_mock_faction(1).await?;
    let faction_model2 = test.eve().insert_mock_faction(2).await?;
    let character_model = test
        .eve()
        .insert_mock_character(1, 1, None, Some(faction_model1.faction_id))
        .await?;

    // Update character with new faction
    let (character_id, character) =
        test.eve()
            .with_mock_character(1, 1, None, Some(faction_model2.faction_id));

    let character_repo = CharacterRepository::new(test.state.db.clone());
    let result = character_repo
        .upsert(
            character_id,
            character,
            character_model.corporation_id,
            Some(faction_model2.id),
        )
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, character_model.id);
    assert_eq!(upserted.faction_id, Some(faction_model2.id));
    assert_ne!(upserted.faction_id, Some(faction_model1.id));

    Ok(())
}

/// Expect Ok when upserting removes faction relationship
#[tokio::test]
async fn removes_faction_relationship_on_upsert() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let faction_model = test.eve().insert_mock_faction(1).await?;
    let character_model = test
        .eve()
        .insert_mock_character(1, 1, None, Some(faction_model.faction_id))
        .await?;

    assert!(character_model.faction_id.is_some());

    // Update character without faction
    let (character_id, character) = test.eve().with_mock_character(1, 1, None, None);

    let character_repo = CharacterRepository::new(test.state.db.clone());
    let result = character_repo
        .upsert(
            character_id,
            character,
            character_model.corporation_id,
            None,
        )
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, character_model.id);
    assert_eq!(upserted.faction_id, None);

    Ok(())
}

/// Expect Error when upserting to a table that doesn't exist
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!()?;
    let (character_id, character) = test.eve().with_mock_character(1, 1, None, None);

    let character_repo = CharacterRepository::new(test.state.db.clone());
    let result = character_repo
        .upsert(character_id, character, 1, None)
        .await;

    assert!(result.is_err());

    Ok(())
}
