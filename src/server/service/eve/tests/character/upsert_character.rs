use chrono::{Duration, Utc};
use sea_orm::{ActiveValue, EntityTrait, IntoActiveModel};

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
    let result = character_service.upsert_character(character_id).await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.character_id, character_id);
    assert!(created.faction_id.is_some());

    faction_endpoint.assert();
    corporation_endpoint.assert();
    character_endpoint.assert();

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
    let result = character_service.upsert_character(character_id).await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.character_id, character_id);
    assert_eq!(created.faction_id, None);

    corporation_endpoint.assert();
    character_endpoint.assert();

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
    let character_model = test
        .eve()
        .insert_mock_character(1, corporation_model1.corporation_id, None, None)
        .await?;

    // Mock endpoint returns character with different corporation
    let corporation_id_2 = 2;
    let (_, mock_corporation_2) = test
        .eve()
        .with_mock_corporation(corporation_id_2, None, None);
    let (_, mock_character) =
        test.eve()
            .with_mock_character(character_model.character_id, corporation_id_2, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);
    let character_endpoint =
        test.eve()
            .with_character_endpoint(character_model.character_id, mock_character, 1);

    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service
        .upsert_character(character_model.character_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, character_model.id);
    assert_ne!(upserted.corporation_id, character_model.corporation_id);

    corporation_endpoint.assert();
    character_endpoint.assert();

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
    let character_model = test
        .eve()
        .insert_mock_character(1, 1, None, Some(1))
        .await?;

    // Set faction last updated before today's faction update window to allow for updating
    // the faction from ESI
    let mut faction_model_am =
        entity::prelude::EveFaction::find_by_id(character_model.faction_id.unwrap())
            .one(&test.state.db)
            .await?
            .unwrap()
            .into_active_model();

    faction_model_am.updated_at = ActiveValue::Set((Utc::now() - Duration::hours(24)).naive_utc());

    entity::prelude::EveFaction::update(faction_model_am)
        .exec(&test.state.db)
        .await?;

    // Mock endpoint returns character with different faction
    let faction_id_2 = 2;
    let corporation_id_2 = 2;
    let mock_faction_2 = test.eve().with_mock_faction(faction_id_2);
    let (_, mock_corporation_2) =
        test.eve()
            .with_mock_corporation(corporation_id_2, None, Some(faction_id_2));
    let (_, mock_character) = test.eve().with_mock_character(
        character_model.character_id,
        corporation_id_2,
        None,
        Some(faction_id_2),
    );

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction_2], 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);
    let character_endpoint =
        test.eve()
            .with_character_endpoint(character_model.character_id, mock_character, 1);

    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service
        .upsert_character(character_model.character_id)
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let upserted = result.unwrap();

    assert_eq!(upserted.id, character_model.id);
    assert_ne!(upserted.faction_id, character_model.faction_id);

    faction_endpoint.assert();
    corporation_endpoint.assert();
    character_endpoint.assert();

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

    // Mock endpoint returns character without faction
    let corporation_id_2 = 2;
    let (_, mock_corporation_2) = test
        .eve()
        .with_mock_corporation(corporation_id_2, None, None);
    let (_, mock_character) =
        test.eve()
            .with_mock_character(character_model.character_id, corporation_id_2, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);
    let character_endpoint =
        test.eve()
            .with_character_endpoint(character_model.character_id, mock_character, 1);

    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service
        .upsert_character(character_model.character_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, character_model.id);
    assert_eq!(upserted.faction_id, None);

    corporation_endpoint.assert();
    character_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting adds faction relationship
#[tokio::test]
async fn adds_faction_relationship_on_upsert() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

    assert_eq!(character_model.faction_id, None);

    // Mock endpoint returns character with faction
    let faction_id = 1;
    let corporation_id_2 = 2;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let (_, mock_corporation_2) =
        test.eve()
            .with_mock_corporation(corporation_id_2, None, Some(faction_id));
    let (_, mock_character) = test.eve().with_mock_character(
        character_model.character_id,
        corporation_id_2,
        None,
        Some(faction_id),
    );

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);
    let character_endpoint =
        test.eve()
            .with_character_endpoint(character_model.character_id, mock_character, 1);

    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service
        .upsert_character(character_model.character_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, character_model.id);
    assert!(upserted.faction_id.is_some());

    faction_endpoint.assert();
    corporation_endpoint.assert();
    character_endpoint.assert();

    Ok(())
}

/// Expect Error when ESI endpoint for character is unavailable
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
    let result = character_service.upsert_character(character_id).await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

/// Expect Error due to required tables not being created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!()?;

    let corporation_id = 1;
    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(1, corporation_id, None, None);

    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
    let result = character_service.upsert_character(character_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));
    character_endpoint.assert();

    Ok(())
}
