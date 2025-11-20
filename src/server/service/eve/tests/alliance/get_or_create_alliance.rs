use super::*;

// Expect Ok with found when alliance exists in database
#[tokio::test]
async fn finds_existing_alliance() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
    let alliance_model = test.eve().insert_mock_alliance(1, None).await?;

    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service
        .get_or_create_alliance(alliance_model.alliance_id)
        .await;

    assert!(result.is_ok());

    Ok(())
}

// Expect Ok when creating new alliance which does not exist in database
#[tokio::test]
async fn creates_alliance_when_missing() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service.get_or_create_alliance(alliance_id).await;

    assert!(result.is_ok());
    alliance_endpoint.assert();

    Ok(())
}

/// Expect Error due to required tables not being created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;

    let alliance_id = 1;
    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service.get_or_create_alliance(alliance_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

// Expect Error when required ESI endpoint is unavailable
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    let alliance_id = 1;
    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service.get_or_create_alliance(alliance_id).await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}
