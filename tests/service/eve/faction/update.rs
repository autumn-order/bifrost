//! Tests for FactionService::update method.
//!
//! This module verifies the faction update service behavior during ESI data
//! synchronization, including caching with 304 Not Modified responses, retry logic
//! for transient failures, and error handling for missing tables or unavailable ESI endpoints.

use bifrost::server::{error::Error, service::eve::faction::FactionService};
use bifrost_test_utils::prelude::*;
use sea_orm::EntityTrait;

/// Tests updating an empty factions table.
///
/// Verifies that the faction service successfully fetches faction data from ESI
/// and populates an empty database table with the retrieved faction records.
///
/// Expected: Ok with one faction inserted
#[tokio::test]
async fn updates_empty_faction_table() -> Result<(), TestError> {
    let faction_id = 1;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .build()
        .await?;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let update_result = faction_service.update().await;

    assert!(update_result.is_ok());
    let updated = update_result.unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].faction_id, faction_id);

    test.assert_mocks();

    Ok(())
}

/// Tests updating factions with fresh data from ESI.
///
/// Verifies that the faction service updates faction records when ESI returns
/// fresh data (200 OK), updating all faction fields and timestamps.
///
/// Expected: Ok with faction updated and new timestamp
#[tokio::test]
async fn updates_factions_with_fresh_data() -> Result<(), TestError> {
    let faction_id = 1;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_mock_faction(faction_id)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .build()
        .await?;

    let faction_before = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_timestamp = faction_before.updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.len(), 1);
    let updated_faction = &updated[0];
    assert!(updated_faction.updated_at > old_timestamp);

    test.assert_mocks();

    Ok(())
}

/// Tests updating timestamps on 304 Not Modified response.
///
/// Verifies that when ESI returns 304 Not Modified, only the updated_at
/// timestamps are updated for all factions while other fields remain unchanged.
///
/// Expected: Ok with empty result list (304 returns no models) and timestamps updated
#[tokio::test]
async fn updates_timestamps_on_304_not_modified() -> Result<(), TestError> {
    let faction_id = 1;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_mock_faction(faction_id)
        .with_mock_endpoint(|server| {
            server
                .mock("GET", "/universe/factions")
                .with_status(304)
                .expect(1)
                .create()
        })
        .build()
        .await?;

    let faction_before = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_timestamp = faction_before.updated_at;
    let old_name = faction_before.name.clone();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert!(updated.is_empty()); // 304 returns empty vec

    // Verify timestamp was updated in database
    let faction_after = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(faction_after.updated_at > old_timestamp);
    assert_eq!(faction_after.name, old_name); // Other fields unchanged

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
    let result = faction_service.update().await;

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
    let result = faction_service.update().await;

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
    let update_result = faction_service.update().await;

    assert!(matches!(
        update_result,
        Err(Error::EsiError(eve_esi::Error::EsiError(_)))
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
    let update_result = faction_service.update().await;

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
    let result = faction_service.update().await;

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
    let result = faction_service.update().await;

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

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update().await;

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

/// Tests updating multiple factions with fresh data.
///
/// Verifies that when multiple factions exist and ESI returns fresh data,
/// all factions are updated with new timestamps.
///
/// Expected: Ok with all factions updated
#[tokio::test]
async fn updates_multiple_factions_with_fresh_data() -> Result<(), TestError> {
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

    let factions_before = entity::prelude::EveFaction::find().all(&test.db).await?;
    let old_timestamps: Vec<_> = factions_before.iter().map(|f| f.updated_at).collect();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.len(), 2);

    // Verify all have newer timestamps
    for faction in updated {
        assert!(old_timestamps
            .iter()
            .all(|old_ts| faction.updated_at > *old_ts));
    }

    test.assert_mocks();

    Ok(())
}

/// Tests handling 304 Not Modified with multiple factions.
///
/// Verifies that when ESI returns 304 for multiple existing factions,
/// all faction timestamps are updated but no data is changed.
///
/// Expected: Ok with empty result list and all timestamps updated
#[tokio::test]
async fn handles_304_with_multiple_factions() -> Result<(), TestError> {
    let faction1_id = 500_001;
    let faction2_id = 500_002;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_mock_faction(faction1_id)
        .with_mock_faction(faction2_id)
        .with_mock_endpoint(|server| {
            server
                .mock("GET", "/universe/factions")
                .with_status(304)
                .expect(1)
                .create()
        })
        .build()
        .await?;

    let factions_before = entity::prelude::EveFaction::find().all(&test.db).await?;
    let old_timestamps: Vec<_> = factions_before.iter().map(|f| f.updated_at).collect();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let faction_service = FactionService::new(&test.db, &test.esi_client);
    let result = faction_service.update().await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert!(updated.is_empty()); // 304 returns empty vec

    // Verify all timestamps were updated
    let factions_after = entity::prelude::EveFaction::find().all(&test.db).await?;
    for faction in factions_after {
        assert!(old_timestamps
            .iter()
            .all(|old_ts| faction.updated_at > *old_ts));
    }

    test.assert_mocks();

    Ok(())
}
