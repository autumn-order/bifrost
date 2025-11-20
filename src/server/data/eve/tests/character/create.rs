use sea_orm::{DbErr, RuntimeErr};

use super::*;

/// Expect success when creating character with a faction ID set
#[tokio::test]
async fn creates_character_with_faction() -> Result<(), TestError> {
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

    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo
        .create(
            character_id,
            character,
            corporation_model.id,
            Some(faction_model.id),
        )
        .await;

    assert!(result.is_ok());
    let created = result.unwrap();

    assert_eq!(created.character_id, character_id);
    assert_eq!(created.faction_id, Some(faction_model.id));

    Ok(())
}

/// Expect success when creating character entry
#[tokio::test]
async fn creates_character_without_faction() -> Result<(), TestError> {
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

    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo
        .create(character_id, character, corporation_model.id, None)
        .await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.character_id, character_id);
    assert_eq!(created.faction_id, None);

    Ok(())
}

/// Expect Error when attempting to create a character without a valid corporation ID set
#[tokio::test]
async fn fails_for_invalid_corporation_id() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let (character_id, character) = test.eve().with_mock_character(1, 1, None, None);

    let corporation_id = 1;
    let character_repo = CharacterRepository::new(&test.state.db);
    let result = character_repo
        .create(character_id, character, corporation_id, None)
        .await;

    assert!(result.is_err());
    // Assert error code is 787 indicating a foreign key constraint error
    assert!(matches!(
        result,
        Err(DbErr::Query(RuntimeErr::SqlxError(err))) if err
            .as_database_error()
            .and_then(|d| d.code().map(|c| c == "787"))
            .unwrap_or(false)
    ));

    Ok(())
}
