use super::*;

/// Expect Ok when creating corporation without alliance or faction
#[tokio::test]
async fn creates_corporation_without_alliance_or_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let (corporation_id, mock_corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service.create_corporation(corporation_id).await;

    assert!(result.is_ok());
    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when creating corporation with alliance
#[tokio::test]
async fn creates_corporation_with_alliance() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let alliance_id = 1;
    let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, None);
    let (corporation_id, mock_corporation) =
        test.eve().with_mock_corporation(1, Some(alliance_id), None);

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service.create_corporation(corporation_id).await;

    assert!(result.is_ok());
    alliance_endpoint.assert();
    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when creating corporation with faction
#[tokio::test]
async fn creates_corporation_with_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let (corporation_id, mock_corporation) =
        test.eve().with_mock_corporation(1, None, Some(faction_id));

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service.create_corporation(corporation_id).await;

    assert!(result.is_ok());
    faction_endpoint.assert();
    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when creating corporation with alliance and faction
#[tokio::test]
async fn creates_corporation_with_alliance_and_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let faction_id = 1;
    let alliance_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, Some(faction_id));
    let (corporation_id, mock_corporation) =
        test.eve()
            .with_mock_corporation(1, Some(alliance_id), Some(faction_id));

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service.create_corporation(corporation_id).await;

    assert!(result.is_ok());
    faction_endpoint.assert();
    alliance_endpoint.assert();
    corporation_endpoint.assert();

    Ok(())
}

/// Expect Error when ESI endpoint is unavailable
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let corporation_id = 1;
    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service.create_corporation(corporation_id).await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

/// Expect Error when database table are not created
#[tokio::test]
async fn fails_for_duplicate_corporation() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

    let (_, mock_corporation) =
        test.eve()
            .with_mock_corporation(corporation_model.corporation_id, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_model.corporation_id, mock_corporation, 1);

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .create_corporation(corporation_model.corporation_id)
        .await;

    assert!(matches!(result, Err(Error::DbErr(_))));
    corporation_endpoint.assert();

    Ok(())
}
