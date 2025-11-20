use super::*;

/// Expect Ok when character is found in database
#[tokio::test]
async fn finds_existing_character() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_or_create_character(character_model.character_id)
        .await;

    assert!(result.is_ok());

    Ok(())
}

/// Expect Ok when character is created when not found in database
#[tokio::test]
async fn creates_character_when_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let corporation_id = 1;
    let (_, mock_corporation) = test.eve().with_mock_corporation(corporation_id, None, None);
    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(1, corporation_id, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);
    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_or_create_character(character_id)
        .await;

    assert!(result.is_ok());
    corporation_endpoint.assert();
    character_endpoint.assert();

    Ok(())
}

/// Expect Error when attempting to access database tables that haven't been created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;

    let character_id = 1;
    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_or_create_character(character_id)
        .await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Expect Error when attempting to fetch from ESI endpoint that doesn't exist
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let character_id = 1;
    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_or_create_character(character_id)
        .await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}
