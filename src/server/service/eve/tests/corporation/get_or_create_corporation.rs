use super::*;

// Expect Ok when corporation is found already present in table
#[tokio::test]
async fn finds_existing_corporation() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .get_or_create_corporation(corporation_model.corporation_id)
        .await;

    assert!(result.is_ok());

    Ok(())
}

// Expect Ok when creating a new corporation when not found in table
#[tokio::test]
async fn creates_corporation_when_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let (corporation_id, mock_corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .get_or_create_corporation(corporation_id)
        .await;

    assert!(result.is_ok());
    corporation_endpoint.assert();

    Ok(())
}

/// Expect Error when trying to access database table that hasn't been created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;

    let corporation_id = 1;
    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .get_or_create_corporation(corporation_id)
        .await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Expect Error when required ESI endpoint is unavailable
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let corporation_id = 1;
    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .get_or_create_corporation(corporation_id)
        .await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}
