use super::*;

/// Expect Ok when creating character without alliance or faction
#[tokio::test]
async fn creates_character_without_alliance_or_faction() -> Result<(), TestError> {
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

    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service.create_character(character_id).await;

    assert!(result.is_ok());
    corporation_endpoint.assert();
    character_endpoint.assert();

    Ok(())
}

/// Expect Ok when creating character with alliance
#[tokio::test]
async fn creates_character_with_alliance() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let alliance_id = 1;
    let corporation_id = 1;
    let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, None);
    let (_, mock_corporation) =
        test.eve()
            .with_mock_corporation(corporation_id, Some(alliance_id), None);
    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(1, corporation_id, Some(alliance_id), None);

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);
    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service.create_character(character_id).await;

    assert!(result.is_ok());
    alliance_endpoint.assert();
    corporation_endpoint.assert();
    character_endpoint.assert();

    Ok(())
}

/// Expect Ok when creating character with faction
#[tokio::test]
async fn creates_character_with_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let faction_id = 1;
    let corporation_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let (_, mock_corporation) =
        test.eve()
            .with_mock_corporation(corporation_id, None, Some(faction_id));
    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(1, corporation_id, None, Some(faction_id));

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);
    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service.create_character(character_id).await;

    assert!(result.is_ok());
    faction_endpoint.assert();
    corporation_endpoint.assert();
    character_endpoint.assert();

    Ok(())
}

/// Expect Ok when creating character with alliance & faction
#[tokio::test]
async fn creates_character_with_alliance_and_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let faction_id = 1;
    let alliance_id = 1;
    let corporation_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, Some(faction_id));
    let (_, mock_corporation) =
        test.eve()
            .with_mock_corporation(corporation_id, Some(alliance_id), Some(faction_id));
    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(1, corporation_id, Some(alliance_id), Some(faction_id));

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);
    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service.create_character(character_id).await;

    assert!(result.is_ok());
    faction_endpoint.assert();
    alliance_endpoint.assert();
    corporation_endpoint.assert();
    character_endpoint.assert();

    Ok(())
}

/// Expect Error when ESI endpoint is unavailable
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let character_id = 1;
    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service.create_character(character_id).await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

/// Expect Error when trying to create character that already exists
#[tokio::test]
async fn fails_for_duplicate_character() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let corporation_id = 1;
    let character_id = 1;
    let _ = test
        .eve()
        .insert_mock_character(character_id, corporation_id, None, None)
        .await?;

    let (_, mock_character) =
        test.eve()
            .with_mock_character(character_id, corporation_id, None, None);

    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service.create_character(character_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));
    character_endpoint.assert();

    Ok(())
}
