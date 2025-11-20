use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};

use crate::server::{
    error::{eve::EveError, Error},
    service::eve::faction::FactionService,
    util::time::effective_faction_cache_expiry,
};

use super::*;

/// Expect Ok with faction found when it is present in database
#[tokio::test]
async fn finds_existing_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
    let faction_model = test.eve().insert_mock_faction(1).await?;

    let mock_faction = test.eve().with_mock_faction(faction_model.faction_id);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 0);

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service
        .get_or_update_factions(faction_model.faction_id)
        .await;

    assert!(result.is_ok());
    let faction = result.unwrap();
    assert_eq!(faction.faction_id, faction_model.faction_id);
    faction_endpoint.assert();

    Ok(())
}

/// Expect Ok with created faction when not present in database
#[tokio::test]
async fn creates_faction_when_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let update_result = faction_service.get_or_update_factions(faction_id).await;

    assert!(update_result.is_ok());
    let faction = update_result.unwrap();
    assert_eq!(faction.faction_id, faction_id);

    // Call method one more time to ensure the faction is not retrieved from endpoint again
    let get_result = faction_service.get_or_update_factions(faction_id).await;
    assert!(get_result.is_ok());
    faction_endpoint.assert();

    Ok(())
}

/// Expect Ok when faction exists and is within cache period, no ESI call made
#[tokio::test]
async fn returns_cached_faction_within_expiry() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
    let faction_model = test.eve().insert_mock_faction(1).await?;

    // Set updated_at to within cache period
    let now = Utc::now();
    let effective_expiry = effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_add_signed(Duration::minutes(1))
        .unwrap_or(effective_expiry);
    let mut faction_am = faction_model.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    let faction_model = faction_am.update(&test.state.db).await?;

    let mock_faction = test.eve().with_mock_faction(faction_model.faction_id);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 0);

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service
        .get_or_update_factions(faction_model.faction_id)
        .await;

    assert!(result.is_ok());
    let faction = result.unwrap();
    assert_eq!(faction.faction_id, faction_model.faction_id);
    assert_eq!(faction.updated_at, updated_at);
    faction_endpoint.assert();

    Ok(())
}

/// Expect Ok when faction not found initially but factions are past cache expiry
#[tokio::test]
async fn updates_factions_past_cache_expiry_when_not_found() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    // Insert a different faction with expired cache
    let existing_faction = test.eve().insert_mock_faction(1).await?;
    let now = Utc::now();
    let effective_expiry = effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_sub_signed(Duration::minutes(5))
        .unwrap_or(effective_expiry);
    let mut faction_am = existing_faction.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.state.db).await?;

    // Request a faction that doesn't exist yet
    let faction_id = 2;
    let mock_faction_1 = test.eve().with_mock_faction(1);
    let mock_faction_2 = test.eve().with_mock_faction(faction_id);
    let faction_endpoint = test
        .eve()
        .with_faction_endpoint(vec![mock_faction_1, mock_faction_2], 1);

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.get_or_update_factions(faction_id).await;

    assert!(result.is_ok());
    let faction = result.unwrap();
    assert_eq!(faction.faction_id, faction_id);
    faction_endpoint.assert();

    Ok(())
}

/// Expect success when ESI fails initially but succeeds on retry
#[tokio::test]
async fn retries_on_esi_server_error() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);

    // First request fails with 500, second succeeds
    let error_endpoint = test
        .server
        .mock("GET", "/universe/factions")
        .with_status(500)
        .expect(1)
        .create();

    let success_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.get_or_update_factions(faction_id).await;

    assert!(result.is_ok());
    let faction = result.unwrap();
    assert_eq!(faction.faction_id, faction_id);

    error_endpoint.assert();
    success_endpoint.assert();

    Ok(())
}

/// Expect error when ESI continuously returns server errors exceeding max retries
#[tokio::test]
async fn fails_after_max_esi_retries() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let faction_id = 1;

    // All 3 attempts fail (as per RetryContext::DEFAULT_MAX_ATTEMPTS)
    let error_endpoint = test
        .server
        .mock("GET", "/universe/factions")
        .with_status(503)
        .expect(3)
        .create();

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.get_or_update_factions(faction_id).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(Error::EsiError(_))));

    error_endpoint.assert();

    Ok(())
}

/// Expect Error when required database tables for factions are missing
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;

    let faction_id = 1;
    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.get_or_update_factions(faction_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Expect Error when ESI endpoint for factions is not available
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let faction_id = 1;
    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.get_or_update_factions(faction_id).await;

    assert!(matches!(
        result,
        Err(Error::EsiError(eve_esi::Error::ReqwestError(_)))
    ));

    Ok(())
}

/// Expect Error if ESI endpoint does not return the required faction
#[tokio::test]
async fn fails_when_faction_not_returned() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let mock_faction = test.eve().with_mock_faction(1);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    let faction_id = 2;
    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.get_or_update_factions(faction_id).await;

    assert!(matches!(
        result,
        Err(Error::EveError(EveError::FactionNotFound(_)))
    ));
    faction_endpoint.assert();

    Ok(())
}

/// Expect Error when faction is not found and factions are already within cache period
#[tokio::test]
async fn fails_when_faction_not_found_within_cache_expiry() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    // Insert an existing faction within cache period
    let existing_faction = test.eve().insert_mock_faction(1).await?;
    let now = Utc::now();
    let effective_expiry = effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_add_signed(Duration::minutes(1))
        .unwrap_or(effective_expiry);
    let mut faction_am = existing_faction.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.state.db).await?;

    let mock_faction = test.eve().with_mock_faction(1);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 0);

    // Request a faction that doesn't exist
    let faction_id = 2;
    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.get_or_update_factions(faction_id).await;

    assert!(matches!(
        result,
        Err(Error::EveError(EveError::FactionNotFound(_)))
    ));
    faction_endpoint.assert();

    Ok(())
}

/// Expect retry logic respects cache expiry check when faction already exists
#[tokio::test]
async fn retry_logic_respects_cache_expiry_for_existing_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
    let faction_model = test.eve().insert_mock_faction(1).await?;

    // Store faction_id before consuming faction_model
    let faction_id = faction_model.faction_id;

    // Set updated_at to within cache period
    let now = Utc::now();
    let effective_expiry = effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_add_signed(Duration::minutes(1))
        .unwrap_or(effective_expiry);
    let mut faction_am = faction_model.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.state.db).await?;

    // Create an endpoint that would fail if called
    let faction_endpoint = test
        .server
        .mock("GET", "/universe/factions")
        .with_status(500)
        .expect(0) // Should not be called
        .create();

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.get_or_update_factions(faction_id).await;

    assert!(result.is_ok());
    let faction = result.unwrap();
    assert_eq!(faction.faction_id, faction_id);

    faction_endpoint.assert();

    Ok(())
}
