use super::*;

/// Expect Ok when fetching & creating an alliance with a faction ID
#[tokio::test]
async fn creates_alliance_with_faction() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, Some(faction_id));

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    let alliance_service =
        AllianceService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = alliance_service.create_alliance(alliance_id).await;

    assert!(result.is_ok());
    faction_endpoint.assert();
    alliance_endpoint.assert();

    Ok(())
}

/// Expect Ok when fetching & creating an alliance without a faction ID
#[tokio::test]
async fn creates_alliance_without_faction() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    let alliance_service =
        AllianceService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = alliance_service.create_alliance(alliance_id).await;

    assert!(result.is_ok());
    alliance_endpoint.assert();

    Ok(())
}

/// Expect Error when ESI endpoint for alliance is unavailable
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    let alliance_id = 1;
    let alliance_service =
        AllianceService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = alliance_service.create_alliance(alliance_id).await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

/// Expect Error when trying to create alliance that already exists
#[tokio::test]
async fn fails_for_duplicate_alliance() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
    let alliance_model = test.eve().insert_mock_alliance(1, None).await?;

    let (_, mock_alliance) = test
        .eve()
        .with_mock_alliance(alliance_model.alliance_id, None);

    let alliance_endpoint =
        test.eve()
            .with_alliance_endpoint(alliance_model.alliance_id, mock_alliance, 1);

    let alliance_service =
        AllianceService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = alliance_service
        .create_alliance(alliance_model.alliance_id)
        .await;

    assert!(matches!(result, Err(Error::DbErr(_))));
    alliance_endpoint.assert();

    Ok(())
}
