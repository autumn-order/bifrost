use super::*;

/// Expect Some when character is present in database
#[tokio::test]
async fn finds_existing_character() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo
        .get_by_character_id(character_model.character_id)
        .await;

    assert!(result.is_ok());
    let character_option = result.unwrap();
    assert!(character_option.is_some());

    Ok(())
}

/// Expect None when no character entry is present
#[tokio::test]
async fn returns_none_for_nonexistent_character() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let character_id = 1;
    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo.get_by_character_id(character_id).await;

    assert!(result.is_ok());
    let character_option = result.unwrap();
    assert!(character_option.is_none());

    Ok(())
}

/// Expect Error when required database tables have not been created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    // Use setup function that doesn't create required tables, causing a database error
    let test = test_setup_with_tables!()?;

    let character_id = 1;
    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo.get_by_character_id(character_id).await;

    assert!(result.is_err());

    Ok(())
}
