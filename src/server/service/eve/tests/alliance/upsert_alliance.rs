use chrono::{Duration, Utc};
use sea_orm::{ActiveValue, EntityTrait, IntoActiveModel};

use super::*;

/// Expect Ok when upserting a new alliance with a faction ID
#[tokio::test]
async fn creates_new_alliance_with_faction() -> Result<(), TestError> {
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
    let result = alliance_service.upsert_alliance(alliance_id).await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.alliance_id, alliance_id);
    assert!(created.faction_id.is_some());

    faction_endpoint.assert();
    alliance_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting a new alliance without a faction ID
#[tokio::test]
async fn creates_new_alliance_without_faction() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    let alliance_service =
        AllianceService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = alliance_service.upsert_alliance(alliance_id).await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.alliance_id, alliance_id);
    assert_eq!(created.faction_id, None);

    alliance_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting an existing alliance and verify it updates
#[tokio::test]
async fn updates_existing_alliance() -> Result<(), TestError> {
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
        .upsert_alliance(alliance_model.alliance_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    // Verify the ID remains the same (it's an update, not a new insert)
    assert_eq!(upserted.id, alliance_model.id);
    assert_eq!(upserted.alliance_id, alliance_model.alliance_id);

    alliance_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting an existing alliance with a new faction ID
#[tokio::test]
async fn updates_alliance_faction_relationship() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
    let faction_model1 = test.eve().insert_mock_faction(1).await?;

    // Set faction last updated before today's faction update window to allow for updating
    // the faction from ESI
    let mut faction_model_am = entity::prelude::EveFaction::find_by_id(faction_model1.id)
        .one(&test.state.db)
        .await?
        .unwrap()
        .into_active_model();

    faction_model_am.updated_at = ActiveValue::Set((Utc::now() - Duration::hours(24)).naive_utc());

    entity::prelude::EveFaction::update(faction_model_am)
        .exec(&test.state.db)
        .await?;

    let alliance_model = test
        .eve()
        .insert_mock_alliance(1, Some(faction_model1.faction_id))
        .await?;

    // Mock endpoint returns alliance with different faction
    let faction_id_2 = 2;
    let mock_faction_2 = test.eve().with_mock_faction(faction_id_2);
    let (_, mock_alliance) = test
        .eve()
        .with_mock_alliance(alliance_model.alliance_id, Some(faction_id_2));

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction_2], 1);
    let alliance_endpoint =
        test.eve()
            .with_alliance_endpoint(alliance_model.alliance_id, mock_alliance, 1);

    let alliance_service =
        AllianceService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = alliance_service
        .upsert_alliance(alliance_model.alliance_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, alliance_model.id);
    assert_ne!(upserted.faction_id, alliance_model.faction_id);

    faction_endpoint.assert();
    alliance_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting removes faction relationship
#[tokio::test]
async fn removes_faction_relationship_on_upsert() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
    let faction_model = test.eve().insert_mock_faction(1).await?;
    let alliance_model = test
        .eve()
        .insert_mock_alliance(1, Some(faction_model.faction_id))
        .await?;

    assert!(alliance_model.faction_id.is_some());

    // Mock endpoint returns alliance without faction
    let (_, mock_alliance) = test
        .eve()
        .with_mock_alliance(alliance_model.alliance_id, None);

    let alliance_endpoint =
        test.eve()
            .with_alliance_endpoint(alliance_model.alliance_id, mock_alliance, 1);

    let alliance_service =
        AllianceService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = alliance_service
        .upsert_alliance(alliance_model.alliance_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, alliance_model.id);
    assert_eq!(upserted.faction_id, None);

    alliance_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting adds faction relationship
#[tokio::test]
async fn adds_faction_relationship_on_upsert() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
    let alliance_model = test.eve().insert_mock_alliance(1, None).await?;

    assert_eq!(alliance_model.faction_id, None);

    // Mock endpoint returns alliance with faction
    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let (_, mock_alliance) = test
        .eve()
        .with_mock_alliance(alliance_model.alliance_id, Some(faction_id));

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
    let alliance_endpoint =
        test.eve()
            .with_alliance_endpoint(alliance_model.alliance_id, mock_alliance, 1);

    let alliance_service =
        AllianceService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = alliance_service
        .upsert_alliance(alliance_model.alliance_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, alliance_model.id);
    assert!(upserted.faction_id.is_some());

    faction_endpoint.assert();
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
    let result = alliance_service.upsert_alliance(alliance_id).await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

/// Expect Error due to required tables not being created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!()?;

    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    let alliance_service =
        AllianceService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = alliance_service.upsert_alliance(alliance_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    alliance_endpoint.assert();

    Ok(())
}
