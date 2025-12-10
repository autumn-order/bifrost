//! Tests for AllianceService::update method.
//!
//! This module verifies the alliance update service behavior during ESI data
//! synchronization, including dependency resolution for factions, retry logic
//! for transient failures, caching with 304 Not Modified responses, and error
//! handling for missing tables or unavailable ESI endpoints.

use bifrost::server::{error::Error, service::eve::alliance::AllianceService};
use bifrost_test_utils::prelude::*;
use sea_orm::EntityTrait;

/// Tests updating a new alliance without faction.
///
/// Verifies that the alliance service successfully fetches alliance data from ESI
/// and creates a new alliance record in the database when the alliance has no
/// faction affiliation.
///
/// Expected: Ok with alliance record created
#[tokio::test]
async fn updates_new_alliance_without_faction() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);
    assert!(alliance.faction_id.is_none());

    // Verify alliance exists in database
    let db_alliance = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_alliance.alliance_id, alliance_id);
    assert!(db_alliance.faction_id.is_none());

    test.assert_mocks();

    Ok(())
}

/// Tests updating a new alliance with faction.
///
/// Verifies that the alliance service successfully resolves faction dependencies,
/// fetches both faction and alliance data from ESI, and creates both records in
/// the correct dependency order.
///
/// Expected: Ok with alliance and faction records created
#[tokio::test]
async fn updates_new_alliance_with_faction() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    // Verify faction exists
    let faction = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(faction.faction_id, faction_id);

    // Verify alliance is linked to faction
    let db_alliance = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_alliance.faction_id, Some(faction.id));

    test.assert_mocks();

    Ok(())
}

/// Tests updating alliance with existing faction.
///
/// Verifies that the alliance service reuses existing faction records instead
/// of creating duplicates when the faction already exists in the database.
///
/// Expected: Ok with alliance linked to existing faction, no duplicate faction created
#[tokio::test]
async fn updates_alliance_with_existing_faction() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .build()
        .await?;

    // Pre-insert faction
    test.eve().insert_mock_faction(faction_id).await?;

    let factions_before = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions_before.len(), 1);

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_ok());

    // Verify no additional faction was created
    let factions_after = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions_after.len(), 1);

    test.assert_mocks();

    Ok(())
}

/// Tests updating existing alliance with fresh data.
///
/// Verifies that the alliance service successfully updates an existing alliance
/// record when fresh data is available from ESI (not 304 Not Modified).
///
/// Expected: Ok with alliance data and timestamp updated
#[tokio::test]
async fn updates_existing_alliance_with_fresh_data() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_mock_alliance(alliance_id, None)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_before = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_updated_at = alliance_before.updated_at;

    // Small delay to ensure timestamp changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    // Verify only one alliance exists
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 1);

    // Verify timestamp was updated
    assert!(alliance.updated_at > old_updated_at);

    test.assert_mocks();

    Ok(())
}

/// Tests updating alliance faction affiliation.
///
/// Verifies that the alliance service correctly updates an alliance's faction_id
/// when the alliance joins or changes faction.
///
/// Expected: Ok with alliance's faction_id updated
#[tokio::test]
async fn updates_alliance_faction_affiliation() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let new_faction_id = 500_002;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_mock_alliance(alliance_id, None)
        .with_faction_endpoint(vec![factory::mock_faction(new_faction_id)], 1)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(new_faction_id)), 1)
        .build()
        .await?;

    let alliance_before = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(alliance_before.faction_id.is_none());

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    // Verify alliance faction was updated
    let db_alliance = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(db_alliance.faction_id.is_some());

    test.assert_mocks();

    Ok(())
}

/// Tests removing alliance faction affiliation.
///
/// Verifies that the alliance service correctly sets an alliance's faction_id
/// to None when the alliance leaves its faction.
///
/// Expected: Ok with alliance's faction_id set to None
#[tokio::test]
async fn removes_alliance_faction_affiliation() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    // Pre-insert alliance with faction
    test.eve()
        .insert_mock_alliance(alliance_id, Some(faction_id))
        .await?;

    let alliance_before = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(alliance_before.faction_id.is_some());

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    // Verify alliance faction was removed
    let db_alliance = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(db_alliance.faction_id.is_none());

    test.assert_mocks();

    Ok(())
}

/// Tests retrying on ESI server error.
///
/// Verifies that the alliance service automatically retries when ESI returns
/// a transient server error (500), and succeeds once ESI recovers.
///
/// Expected: Ok with alliance created after retry
#[tokio::test]
async fn retries_on_esi_server_error() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    test.assert_mocks();

    Ok(())
}

/// Tests updating with faction dependency.
///
/// Verifies that the alliance service correctly fetches faction dependencies
/// when updating an alliance that has a faction affiliation.
///
/// Expected: Ok with both faction and alliance created
#[tokio::test]
async fn updates_with_faction_dependency() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_ok());

    // Verify both faction and alliance exist
    let faction = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(faction.faction_id, faction_id);

    let alliance = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    test.assert_mocks();

    Ok(())
}

/// Tests failure after max ESI retries.
///
/// Verifies that the alliance service returns an error after exhausting all
/// retry attempts (3 attempts) when ESI continues to return server errors.
///
/// Expected: Err(Error::EsiError)
#[tokio::test]
async fn fails_after_max_esi_retries() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint_error(alliance_id, 500, 3)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_err());
    match result {
        Err(Error::EsiError(_)) => (),
        _ => panic!("Expected Error::EsiError, got: {:?}", result),
    }

    test.assert_mocks();

    Ok(())
}

/// Tests failure when ESI is unavailable.
///
/// Verifies that the alliance service returns an error when ESI returns
/// a client error (404) indicating the alliance doesn't exist.
///
/// Expected: Err(Error::EsiError)
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint_error(alliance_id, 404, 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_err());

    test.assert_mocks();

    Ok(())
}

/// Tests failure when tables are missing.
///
/// Verifies that the alliance service returns a database error when attempting
/// to insert into a non-existent alliance table.
///
/// Expected: Err(Error::DbErr)
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 0)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_err());
    match result {
        Err(Error::DbErr(_)) => (),
        _ => panic!("Expected Error::DbErr, got: {:?}", result),
    }

    Ok(())
}

/// Tests failure when faction table is missing.
///
/// Verifies that the alliance service returns a database error when attempting
/// to insert an alliance with a faction but the faction table doesn't exist.
///
/// Expected: Err(Error::DbErr)
#[tokio::test]
async fn fails_when_faction_table_missing() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveAlliance)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 0)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 0)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_err());
    match result {
        Err(Error::DbErr(_)) => (),
        _ => panic!("Expected Error::DbErr, got: {:?}", result),
    }

    Ok(())
}

/// Tests transaction rollback on error.
///
/// Verifies that database changes are rolled back when an error occurs during
/// the update operation, ensuring data consistency.
///
/// Expected: Err with no partial data in database
#[tokio::test]
async fn rolls_back_transaction_on_error() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint_error(alliance_id, 500, 3)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let _result = alliance_service.update(alliance_id).await;

    // Verify no alliance was created
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 0);

    test.assert_mocks();

    Ok(())
}

/// Tests updating multiple alliances sequentially.
///
/// Verifies that the alliance service can successfully update multiple different
/// alliances in sequence, with each update being independent.
///
/// Expected: Ok with all three alliances created
#[tokio::test]
async fn updates_multiple_alliances_sequentially() -> Result<(), TestError> {
    let alliance_id_1 = 99_000_001;
    let alliance_id_2 = 99_000_002;
    let alliance_id_3 = 99_000_003;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint(alliance_id_1, factory::mock_alliance(None), 1)
        .with_alliance_endpoint(alliance_id_2, factory::mock_alliance(None), 1)
        .with_alliance_endpoint(alliance_id_3, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);

    let result1 = alliance_service.update(alliance_id_1).await;
    assert!(result1.is_ok());

    let result2 = alliance_service.update(alliance_id_2).await;
    assert!(result2.is_ok());

    let result3 = alliance_service.update(alliance_id_3).await;
    assert!(result3.is_ok());

    // Verify all three alliances exist
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 3);

    test.assert_mocks();

    Ok(())
}

/// Tests that update is idempotent.
///
/// Verifies that calling update multiple times for the same alliance doesn't
/// create duplicate records and updates the existing record each time.
///
/// Expected: Ok with only one alliance record, timestamp updated on each call
#[tokio::test]
async fn update_is_idempotent() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 3)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);

    let result1 = alliance_service.update(alliance_id).await;
    assert!(result1.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let result2 = alliance_service.update(alliance_id).await;
    assert!(result2.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let result3 = alliance_service.update(alliance_id).await;
    assert!(result3.is_ok());

    // Verify only one alliance exists
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 1);

    // Verify timestamp increased with each update
    assert!(result2.unwrap().updated_at > result1.unwrap().updated_at);

    test.assert_mocks();

    Ok(())
}

/// Tests updating existing alliance with 304 Not Modified response.
///
/// Verifies that when ESI returns 304 Not Modified (indicating data hasn't changed),
/// the service only updates the updated_at timestamp without fetching fresh data
/// or modifying other fields.
///
/// Expected: Ok with only timestamp updated, no ESI fetch for alliance data
#[tokio::test]
async fn updates_timestamp_on_304_not_modified() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_mock_alliance(alliance_id, None)
        .with_alliance_endpoint_not_modified(alliance_id, 1)
        .build()
        .await?;

    let alliance_before = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_timestamp = alliance_before.updated_at;
    let old_name = alliance_before.name.clone();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();

    // Verify timestamp was updated
    assert!(alliance.updated_at > old_timestamp);

    // Verify other fields remained unchanged
    assert_eq!(alliance.name, old_name);
    assert_eq!(alliance.alliance_id, alliance_id);

    test.assert_mocks();

    Ok(())
}

/// Tests update with 304 Not Modified for alliance with faction.
///
/// Verifies that 304 Not Modified handling works correctly for alliances
/// with faction affiliations, preserving those relationships.
///
/// Expected: Ok with timestamp updated, faction preserved
#[tokio::test]
async fn updates_timestamp_on_304_with_faction() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint_not_modified(alliance_id, 1)
        .build()
        .await?;

    // Pre-insert alliance with faction
    test.eve()
        .insert_mock_alliance(alliance_id, Some(faction_id))
        .await?;

    let alliance_before = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_timestamp = alliance_before.updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.update(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();

    // Verify timestamp was updated
    assert!(alliance.updated_at > old_timestamp);

    // Verify faction was preserved
    assert_eq!(alliance.faction_id, alliance_before.faction_id);

    test.assert_mocks();

    Ok(())
}

/// Tests multiple updates with alternating 304 and fresh responses.
///
/// Verifies that the service correctly handles a mix of 304 Not Modified and
/// fresh data responses across multiple update calls for the same alliance.
///
/// Expected: Ok with appropriate behavior for each response type
#[tokio::test]
async fn handles_mixed_304_and_fresh_responses() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .with_alliance_endpoint_not_modified(alliance_id, 1)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);

    // First update: creates new alliance
    let result1 = alliance_service.update(alliance_id).await;
    assert!(result1.is_ok());
    let alliance1 = result1.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Second update: 304 Not Modified
    let result2 = alliance_service.update(alliance_id).await;
    assert!(result2.is_ok());
    let alliance2 = result2.unwrap();
    assert!(alliance2.updated_at > alliance1.updated_at);

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Third update: fresh data
    let result3 = alliance_service.update(alliance_id).await;
    assert!(result3.is_ok());
    let alliance3 = result3.unwrap();
    assert!(alliance3.updated_at > alliance2.updated_at);

    // Verify only one alliance exists
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 1);

    test.assert_mocks();

    Ok(())
}
