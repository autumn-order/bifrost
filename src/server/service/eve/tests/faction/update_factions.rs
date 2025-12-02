use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};

use crate::server::{
    error::Error,
    service::{eve::faction::FactionService, orchestrator::faction::FactionOrchestrator},
};

use super::*;

/// Expect success when updating an empty factions table
#[tokio::test]
async fn updates_empty_faction_table() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let update_result = faction_service.update_factions().await;

    assert!(update_result.is_ok());
    let updated = update_result.unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].faction_id, faction_id);

    faction_endpoint.assert();

    Ok(())
}

/// Expect Ok with an update performed due to existing factions being past cache expiry
#[tokio::test]
async fn updates_factions_past_cache_expiry() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
    let faction_model = test.eve().insert_mock_faction(1).await?;

    let mock_faction = test.eve().with_mock_faction(faction_model.faction_id);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    // Set updated_at to *before* the effective expiry so an update should be performed.
    let now = Utc::now();
    let effective_expiry = FactionOrchestrator::effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_sub_signed(Duration::minutes(5))
        .unwrap_or(effective_expiry);
    let mut faction_am = faction_model.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.state.db).await?;

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.len(), 1);
    let updated_faction = updated.iter().next().unwrap();
    assert!(updated_faction.updated_at > updated_at);

    faction_endpoint.assert();

    Ok(())
}

/// Expect Ok with no update performed due to existing factions still being within cache period
#[tokio::test]
async fn skips_update_within_cache_expiry() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
    let faction_model = test.eve().insert_mock_faction(1).await?;

    let mock_faction = test.eve().with_mock_faction(faction_model.faction_id);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 0);

    // Set updated_at to just after the effective expiry so it should be considered cached.
    let now = Utc::now();
    let effective_expiry = FactionOrchestrator::effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_add_signed(Duration::minutes(1))
        .unwrap_or(effective_expiry);
    let mut faction_am = faction_model.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.state.db).await?;

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert!(updated.is_empty());

    faction_endpoint.assert();

    Ok(())
}

/// Expect success when ESI fails initially but succeeds on retry with cached data reused
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
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].faction_id, faction_id);

    error_endpoint.assert();
    success_endpoint.assert();

    Ok(())
}

/// Expect success when ESI succeeds but initial database operation would fail
/// The retry should reuse cached ESI data without fetching again
#[tokio::test]
// TODO: we need a way to make DB fail on first insertion attempt
#[ignore]
async fn reuses_cached_data_on_retry() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);

    // ESI should only be called once - data is cached for any retries
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.len(), 1);

    // Verify ESI was only called once (cached data would be reused on DB retry)
    faction_endpoint.assert();

    Ok(())
}

/// Expect error when ESI continuously returns server errors exceeding max retries
#[tokio::test]
async fn fails_after_max_esi_retries() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    // All 3 attempts fail (as per RetryContext::DEFAULT_MAX_ATTEMPTS)
    let error_endpoint = test
        .server
        .mock("GET", "/universe/factions")
        .with_status(503)
        .expect(3)
        .create();

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_err());
    assert!(matches!(result, Err(Error::EsiError(_))));

    error_endpoint.assert();

    Ok(())
}

/// Expect error when ESI endpoint is completely unavailable (connection refused)
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    // No mock endpoint is created, so connection will be refused
    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let update_result = faction_service.update_factions().await;

    assert!(matches!(
        update_result,
        Err(Error::EsiError(eve_esi::Error::ReqwestError(_)))
    ));

    Ok(())
}

/// Expect error when attempting to update factions due to required tables not being created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!()?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);

    // ESI will be called but database operations will fail
    let _faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
    let update_result = faction_service.update_factions().await;

    assert!(matches!(update_result, Err(Error::DbErr(_))));

    Ok(())
}

/// Expect retry logic respects cache expiry check (early return before ESI call)
#[tokio::test]
async fn retry_logic_respects_cache_expiry_early_return() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
    let faction_model = test.eve().insert_mock_faction(1).await?;

    // Set updated_at to within cache period
    let now = Utc::now();
    let effective_expiry = FactionOrchestrator::effective_faction_cache_expiry(now).unwrap();
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
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert!(updated.is_empty()); // No update performed due to cache

    faction_endpoint.assert();

    Ok(())
}
