use chrono::{Duration, Utc};
use sea_orm::{ActiveValue, EntityTrait, IntoActiveModel};

use super::*;

/// Expect Ok when upserting a new corporation with alliance and faction
#[tokio::test]
async fn creates_new_corporation_with_alliance_and_faction() -> Result<(), TestError> {
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

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service.upsert_corporation(corporation_id).await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.corporation_id, corporation_id);
    assert!(created.alliance_id.is_some());
    assert!(created.faction_id.is_some());

    faction_endpoint.assert();
    alliance_endpoint.assert();
    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting a new corporation without alliance or faction
#[tokio::test]
async fn creates_new_corporation_without_alliance_or_faction() -> Result<(), TestError> {
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
    let result = corporation_service.upsert_corporation(corporation_id).await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.corporation_id, corporation_id);
    assert_eq!(created.alliance_id, None);
    assert_eq!(created.faction_id, None);

    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting an existing corporation and verify it updates
#[tokio::test]
async fn updates_existing_corporation() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

    let (_, mock_corporation) =
        test.eve()
            .with_mock_corporation(corporation_model.corporation_id, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_model.corporation_id, mock_corporation, 1);

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .upsert_corporation(corporation_model.corporation_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    // Verify the ID remains the same (it's an update, not a new insert)
    assert_eq!(upserted.id, corporation_model.id);
    assert_eq!(upserted.corporation_id, corporation_model.corporation_id);

    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting an existing corporation with a new alliance ID
#[tokio::test]
async fn updates_corporation_alliance_relationship() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let alliance_model1 = test.eve().insert_mock_alliance(1, None).await?;
    let corporation_model = test
        .eve()
        .insert_mock_corporation(1, Some(alliance_model1.alliance_id), None)
        .await?;

    // Mock endpoint returns corporation with different alliance
    let alliance_id_2 = 2;
    let (_, mock_alliance_2) = test.eve().with_mock_alliance(alliance_id_2, None);
    let (_, mock_corporation) = test.eve().with_mock_corporation(
        corporation_model.corporation_id,
        Some(alliance_id_2),
        None,
    );

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id_2, mock_alliance_2, 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_model.corporation_id, mock_corporation, 1);

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .upsert_corporation(corporation_model.corporation_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, corporation_model.id);
    assert_ne!(upserted.alliance_id, corporation_model.alliance_id);

    alliance_endpoint.assert();
    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting an existing corporation with a new faction ID
#[tokio::test]
async fn updates_corporation_faction_relationship() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
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

    let corporation_model = test
        .eve()
        .insert_mock_corporation(1, None, Some(faction_model1.faction_id))
        .await?;

    // Mock endpoint returns corporation with different faction
    let faction_id_2 = 2;
    let mock_faction_2 = test.eve().with_mock_faction(faction_id_2);
    let (_, mock_corporation) = test.eve().with_mock_corporation(
        corporation_model.corporation_id,
        None,
        Some(faction_id_2),
    );

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction_2], 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_model.corporation_id, mock_corporation, 1);

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .upsert_corporation(corporation_model.corporation_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, corporation_model.id);
    assert_ne!(upserted.faction_id, corporation_model.faction_id);

    faction_endpoint.assert();
    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting removes alliance relationship
#[tokio::test]
async fn removes_alliance_relationship_on_upsert() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
    let corporation_model = test
        .eve()
        .insert_mock_corporation(1, Some(alliance_model.alliance_id), None)
        .await?;

    assert!(corporation_model.alliance_id.is_some());

    // Mock endpoint returns corporation without alliance
    let (_, mock_corporation) =
        test.eve()
            .with_mock_corporation(corporation_model.corporation_id, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_model.corporation_id, mock_corporation, 1);

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .upsert_corporation(corporation_model.corporation_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, corporation_model.id);
    assert_eq!(upserted.alliance_id, None);

    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting removes faction relationship
#[tokio::test]
async fn removes_faction_relationship_on_upsert() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let faction_model = test.eve().insert_mock_faction(1).await?;
    let corporation_model = test
        .eve()
        .insert_mock_corporation(1, None, Some(faction_model.faction_id))
        .await?;

    assert!(corporation_model.faction_id.is_some());

    // Mock endpoint returns corporation without faction
    let (_, mock_corporation) =
        test.eve()
            .with_mock_corporation(corporation_model.corporation_id, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_model.corporation_id, mock_corporation, 1);

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .upsert_corporation(corporation_model.corporation_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, corporation_model.id);
    assert_eq!(upserted.faction_id, None);

    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting adds alliance relationship
#[tokio::test]
async fn adds_alliance_relationship_on_upsert() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

    assert_eq!(corporation_model.alliance_id, None);

    // Mock endpoint returns corporation with alliance
    let alliance_id = 1;
    let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, None);
    let (_, mock_corporation) =
        test.eve()
            .with_mock_corporation(corporation_model.corporation_id, Some(alliance_id), None);

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_model.corporation_id, mock_corporation, 1);

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .upsert_corporation(corporation_model.corporation_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, corporation_model.id);
    assert!(upserted.alliance_id.is_some());

    alliance_endpoint.assert();
    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when upserting adds faction relationship
#[tokio::test]
async fn adds_faction_relationship_on_upsert() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

    assert_eq!(corporation_model.faction_id, None);

    // Mock endpoint returns corporation with faction
    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let (_, mock_corporation) =
        test.eve()
            .with_mock_corporation(corporation_model.corporation_id, None, Some(faction_id));

    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_model.corporation_id, mock_corporation, 1);

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service
        .upsert_corporation(corporation_model.corporation_id)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, corporation_model.id);
    assert!(upserted.faction_id.is_some());

    faction_endpoint.assert();
    corporation_endpoint.assert();

    Ok(())
}

/// Expect Error when ESI endpoint for corporation is unavailable
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
    let result = corporation_service.upsert_corporation(corporation_id).await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

/// Expect Error due to required tables not being created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!()?;

    let (corporation_id, mock_corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let corporation_service =
        CorporationService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = corporation_service.upsert_corporation(corporation_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    corporation_endpoint.assert();

    Ok(())
}
