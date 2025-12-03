//! Tests for FactionService::update_factions method.
//!
//! This module verifies the faction update service behavior during ESI data
//! synchronization, including cache expiry handling, retry logic for transient
//! failures, and error handling for missing tables or unavailable ESI endpoints.

use bifrost::server::{
    error::Error,
    service::{eve::faction::FactionService, orchestrator::faction::FactionOrchestrator},
};
use bifrost_test_utils::prelude::*;
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, IntoActiveModel};

/// Tests updating an empty factions table.
///
/// Verifies that the faction service successfully fetches faction data from ESI
/// and populates an empty database table with the retrieved faction records.
///
/// Expected: Ok with one faction inserted</parameter>
#[tokio::test]
async fn updates_empty_faction_table() -> Result<(), TestError> {
    let faction_id = 1;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .build()
        .await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let update_result = faction_service.update_factions().await;

    assert!(update_result.is_ok());
    let updated = update_result.unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].faction_id, faction_id);

    test.assert_mocks();

    Ok(())
}

/// Tests updating factions past cache expiry.
///
/// Verifies that the faction service updates faction records when their
/// updated_at timestamp exceeds the cache duration, triggering a fresh fetch
/// from ESI and database update.
///
/// Expected: Ok with faction updated and new timestamp
#[tokio::test]
async fn updates_factions_past_cache_expiry() -> Result<(), TestError> {
    let faction_id = 1;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_mock_faction(faction_id)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .build()
        .await?;

    let faction_model = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();

    // Set updated_at to *before* the effective expiry so an update should be performed.
    let now = Utc::now();
    let effective_expiry = FactionOrchestrator::effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_sub_signed(Duration::minutes(5))
        .unwrap_or(effective_expiry);
    let mut faction_am = faction_model.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.db).await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.len(), 1);
    let updated_faction = updated.iter().next().unwrap();
    assert!(updated_faction.updated_at > updated_at);

    test.assert_mocks();

    Ok(())
}

/// Tests skipping update when within cache period.
///
/// Verifies that the faction service skips ESI calls and database updates when
/// existing faction records have recent updated_at timestamps within the cache
/// expiry window.
///
/// Expected: Ok with empty update list and no ESI calls
#[tokio::test]
async fn skips_update_within_cache_expiry() -> Result<(), TestError> {
    let faction_id = 1;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_mock_faction(faction_id)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 0)
        .build()
        .await?;

    let faction_model = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();

    // Set updated_at to just after the effective expiry so it should be considered cached.
    let now = Utc::now();
    let effective_expiry = FactionOrchestrator::effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_add_signed(Duration::minutes(1))
        .unwrap_or(effective_expiry);
    let mut faction_am = faction_model.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.db).await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert!(updated.is_empty());

    test.assert_mocks();

    Ok(())
}

/// Tests retry logic on ESI server error.
///
/// Verifies that the faction service successfully retries failed ESI requests
/// when encountering transient server errors (500), ultimately succeeding and
/// updating faction data.
///
/// Expected: Ok with faction data updated after retry
#[tokio::test]
async fn retries_on_esi_server_error() -> Result<(), TestError> {
    let faction_id = 1;

    // First request fails with 500, second succeeds
    // Note: Mockito matches mocks in creation order for the same path
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_mock_endpoint(|server| {
            server
                .mock("GET", "/universe/factions")
                .with_status(500)
                .expect(1)
                .create()
        })
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .build()
        .await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].faction_id, faction_id);

    test.assert_mocks();

    Ok(())
}

/// Tests error handling after max retries.
///
/// Verifies that the faction service returns an error when ESI continuously
/// returns server errors (503) exceeding the maximum retry attempt limit.
///
/// Expected: Err with EsiError after 3 failed attempts
#[tokio::test]
async fn fails_after_max_esi_retries() -> Result<(), TestError> {
    // All 3 attempts fail (as per RetryContext::DEFAULT_MAX_ATTEMPTS)
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_mock_endpoint(|server| {
            server
                .mock("GET", "/universe/factions")
                .with_status(503)
                .expect(3)
                .create()
        })
        .build()
        .await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_err());
    assert!(matches!(result, Err(Error::EsiError(_))));

    test.assert_mocks();

    Ok(())
}

/// Tests error handling when ESI is unavailable.
///
/// Verifies that the faction service returns an error when the ESI endpoint
/// is completely unreachable due to connection failures.
///
/// Expected: Err with ReqwestError
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    // No mock endpoint is created, so connection will be refused
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .build()
        .await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let update_result = faction_service.update_factions().await;

    assert!(matches!(
        update_result,
        Err(Error::EsiError(eve_esi::Error::ReqwestError(_)))
    ));

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the faction service returns a database error when attempting
/// to update factions without the required database tables being created.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let faction_id = 1;

    // ESI will be called but database operations will fail
    let test = TestBuilder::new()
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .build()
        .await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let update_result = faction_service.update_factions().await;

    assert!(matches!(update_result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests updating multiple factions at once.
///
/// Verifies that the faction service correctly handles multiple factions in a single
/// ESI response, creating or updating all faction records in the database.
///
/// Expected: Ok with all factions updated
#[tokio::test]
async fn updates_multiple_factions() -> Result<(), TestError> {
    let faction1_id = 500_001;
    let faction2_id = 500_002;
    let faction3_id = 500_003;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_faction_endpoint(
            vec![
                factory::mock_faction(faction1_id),
                factory::mock_faction(faction2_id),
                factory::mock_faction(faction3_id),
            ],
            1,
        )
        .build()
        .await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.len(), 3);

    // Verify all factions exist in database
    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions.len(), 3);

    let faction_ids: Vec<i64> = factions.iter().map(|f| f.faction_id).collect();
    assert!(faction_ids.contains(&faction1_id));
    assert!(faction_ids.contains(&faction2_id));
    assert!(faction_ids.contains(&faction3_id));

    test.assert_mocks();

    Ok(())
}

/// Tests handling empty ESI response.
///
/// Verifies that the faction service gracefully handles cases where ESI returns
/// an empty faction list, returning an empty update list without errors.
///
/// Expected: Ok with empty update list
#[tokio::test]
async fn handles_empty_esi_response() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_faction_endpoint(vec![], 1)
        .build()
        .await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert!(updated.is_empty());

    test.assert_mocks();

    Ok(())
}

/// Tests adding new factions from ESI.
///
/// Verifies that when ESI returns factions not present in the database, the faction
/// service creates new records for them alongside updating existing ones.
///
/// Expected: Ok with both existing faction updated and new faction created
#[tokio::test]
async fn adds_new_factions_from_esi() -> Result<(), TestError> {
    let existing_faction_id = 500_001;
    let new_faction_id = 500_002;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_mock_faction(existing_faction_id)
        .with_faction_endpoint(
            vec![
                factory::mock_faction(existing_faction_id),
                factory::mock_faction(new_faction_id),
            ],
            1,
        )
        .build()
        .await?;

    // Set existing faction to expired so it gets updated
    let existing_faction = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();
    let now = Utc::now();
    let effective_expiry = FactionOrchestrator::effective_faction_cache_expiry(now).unwrap();
    let expired_updated_at = effective_expiry
        .checked_sub_signed(Duration::minutes(5))
        .unwrap_or(effective_expiry);
    let mut faction_am = existing_faction.into_active_model();
    faction_am.updated_at = ActiveValue::Set(expired_updated_at);
    faction_am.update(&test.db).await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.len(), 2); // Both existing (updated) and new (created)

    // Verify both factions exist in database
    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions.len(), 2);

    let faction_ids: Vec<i64> = factions.iter().map(|f| f.faction_id).collect();
    assert!(faction_ids.contains(&existing_faction_id));
    assert!(faction_ids.contains(&new_faction_id));

    test.assert_mocks();

    Ok(())
}

/// Tests updating all factions when all are expired.
///
/// Verifies that when all factions in the database have expired cache entries,
/// the service updates all of them in a single operation.
///
/// Expected: Ok with all factions updated
#[tokio::test]
async fn updates_all_factions_when_all_expired() -> Result<(), TestError> {
    let faction1_id = 500_001;
    let faction2_id = 500_002;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_mock_faction(faction1_id)
        .with_mock_faction(faction2_id)
        .with_faction_endpoint(
            vec![
                factory::mock_faction(faction1_id),
                factory::mock_faction(faction2_id),
            ],
            1,
        )
        .build()
        .await?;

    // Set both factions to expired
    let now = Utc::now();
    let effective_expiry = FactionOrchestrator::effective_faction_cache_expiry(now).unwrap();
    let expired_updated_at = effective_expiry
        .checked_sub_signed(Duration::minutes(5))
        .unwrap_or(effective_expiry);

    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    for faction in factions {
        let mut faction_am = faction.into_active_model();
        faction_am.updated_at = ActiveValue::Set(expired_updated_at);
        faction_am.update(&test.db).await?;
    }

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.len(), 2);

    // Verify all have new timestamps
    for faction in updated {
        assert!(faction.updated_at > expired_updated_at);
    }

    test.assert_mocks();

    Ok(())
}

/// Tests retry logic respects cache expiry early return.
///
/// Verifies that the faction service's cache expiry check occurs before ESI
/// calls, preventing unnecessary API requests even during retry scenarios when
/// cached data is still valid.
///
/// Expected: Ok with no ESI calls and empty update list
#[tokio::test]
async fn retry_logic_respects_cache_expiry_early_return() -> Result<(), TestError> {
    let faction_id = 1;

    // Create an endpoint that would fail if called
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_mock_faction(faction_id)
        .with_mock_endpoint(|server| {
            server
                .mock("GET", "/universe/factions")
                .with_status(500)
                .expect(0) // Should not be called
                .create()
        })
        .build()
        .await?;

    let faction_model = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();

    // Set updated_at to within cache period
    let now = Utc::now();
    let effective_expiry = FactionOrchestrator::effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_add_signed(Duration::minutes(1))
        .unwrap_or(effective_expiry);
    let mut faction_am = faction_model.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.db).await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update_factions().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert!(updated.is_empty()); // No update performed due to cache

    test.assert_mocks();

    Ok(())
}
