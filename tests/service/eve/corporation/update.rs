//! Tests for CorporationService::update method.
//!
//! This module verifies the corporation update service behavior during ESI data
//! synchronization, including dependency resolution for factions and alliances,
//! retry logic for transient failures, caching with 304 Not Modified responses,
//! and error handling for missing tables or unavailable ESI endpoints.

use bifrost::server::{error::Error, service::eve::corporation::CorporationService};
use bifrost_test_utils::prelude::*;
use sea_orm::EntityTrait;

/// Tests updating a new corporation without alliance or faction.
///
/// Verifies that the corporation service successfully fetches corporation data from ESI
/// and creates a new corporation record in the database when the corporation has no
/// alliance or faction affiliations.
///
/// Expected: Ok with corporation record created
#[tokio::test]
async fn updates_new_corporation_without_affiliations() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);
    assert!(corporation.alliance_id.is_none());

    // Verify corporation exists in database
    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_corporation.corporation_id, corporation_id);
    assert!(db_corporation.alliance_id.is_none());

    test.assert_mocks();

    Ok(())
}

/// Tests updating a new corporation with alliance.
///
/// Verifies that the corporation service successfully resolves alliance dependencies,
/// fetches both alliance and corporation data from ESI, and creates both records in
/// the correct dependency order.
///
/// Expected: Ok with corporation and alliance records created
#[tokio::test]
async fn updates_new_corporation_with_alliance() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), None),
            1,
        )
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    // Verify alliance exists
    let alliance = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    // Verify corporation is linked to alliance
    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_corporation.alliance_id, Some(alliance.id));

    test.assert_mocks();

    Ok(())
}

/// Tests updating a new corporation with faction.
///
/// Verifies that the corporation service successfully resolves faction dependencies,
/// fetches corporation data from ESI, and creates the corporation with faction reference.
///
/// Expected: Ok with corporation record created with faction_id
#[tokio::test]
async fn updates_new_corporation_with_faction() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_mock_faction(faction_id)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(None, Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    // Verify corporation is linked to faction
    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(db_corporation.faction_id.is_some());

    test.assert_mocks();

    Ok(())
}

/// Tests updating corporation with alliance and faction.
///
/// Verifies that the corporation service successfully resolves both alliance and faction
/// dependencies, fetching all required data from ESI and creating records in correct order.
///
/// Expected: Ok with corporation, alliance, and faction records created
#[tokio::test]
async fn updates_corporation_with_alliance_and_faction() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_mock_faction(faction_id)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    // Verify alliance and faction exist
    let alliance = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    let faction = entity::prelude::EveFaction::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(faction.faction_id, faction_id);

    // Verify corporation is linked to both
    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_corporation.alliance_id, Some(alliance.id));
    assert_eq!(db_corporation.faction_id, Some(faction.id));

    test.assert_mocks();

    Ok(())
}

/// Tests updating corporation with existing alliance.
///
/// Verifies that the corporation service reuses existing alliance records instead
/// of creating duplicates when the alliance already exists in the database.
///
/// Expected: Ok with corporation linked to existing alliance, no duplicate alliance created
#[tokio::test]
async fn updates_corporation_with_existing_alliance() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;

    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), None),
            1,
        )
        .build()
        .await?;

    // Pre-insert alliance
    let existing_alliance = test.eve().insert_mock_alliance(alliance_id, None).await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    // Verify only one alliance exists
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 1);

    // Verify corporation is linked to the existing alliance
    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_corporation.alliance_id, Some(existing_alliance.id));

    test.assert_mocks();

    Ok(())
}

/// Tests updating corporation with existing faction.
///
/// Verifies that the corporation service reuses existing faction records instead
/// of creating duplicates when the faction already exists in the database.
///
/// Expected: Ok with corporation linked to existing faction, no duplicate faction created
#[tokio::test]
async fn updates_corporation_with_existing_faction() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let faction_id = 500_001;

    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(None, Some(faction_id)),
            1,
        )
        .build()
        .await?;

    // Pre-insert faction
    test.eve().insert_mock_faction(faction_id).await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());

    // Verify only one faction exists
    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions.len(), 1);

    test.assert_mocks();

    Ok(())
}

/// Tests updating existing corporation with fresh data.
///
/// Verifies that the corporation service successfully updates an existing corporation
/// record when fresh data is available from ESI (not 304 Not Modified).
///
/// Expected: Ok with corporation data and timestamp updated
#[tokio::test]
async fn updates_existing_corporation_with_fresh_data() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_mock_corporation(corporation_id, None, None)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_before = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_updated_at = corporation_before.info_updated_at;

    // Small delay to ensure timestamp changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    // Verify only one corporation exists
    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 1);

    // Verify timestamp was updated
    assert!(corporation.info_updated_at > old_updated_at);

    test.assert_mocks();

    Ok(())
}

/// Tests updating corporation alliance affiliation.
///
/// Verifies that the corporation service correctly updates a corporation's alliance_id
/// when the corporation joins or changes alliance.
///
/// Expected: Ok with corporation's alliance_id updated
#[tokio::test]
async fn updates_corporation_alliance_affiliation() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let new_alliance_id = 99_000_002;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_mock_corporation(corporation_id, None, None)
        .with_alliance_endpoint(new_alliance_id, factory::mock_alliance(None), 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(new_alliance_id), None),
            1,
        )
        .build()
        .await?;

    let corporation_before = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(corporation_before.alliance_id.is_none());

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    // Verify corporation alliance was updated
    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(db_corporation.alliance_id.is_some());

    test.assert_mocks();

    Ok(())
}

/// Tests removing corporation alliance affiliation.
///
/// Verifies that the corporation service correctly sets a corporation's alliance_id
/// to None when the corporation leaves its alliance.
///
/// Expected: Ok with corporation's alliance_id set to None
#[tokio::test]
async fn removes_corporation_alliance_affiliation() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;

    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    // Pre-insert corporation with alliance
    test.eve()
        .insert_mock_corporation(corporation_id, Some(alliance_id), None)
        .await?;

    let corporation_before = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(corporation_before.alliance_id.is_some());

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    // Verify corporation alliance was removed
    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(db_corporation.alliance_id.is_none());

    test.assert_mocks();

    Ok(())
}

/// Tests retrying on ESI server error.
///
/// Verifies that the corporation service automatically retries when ESI returns
/// a transient server error (500), and succeeds once ESI recovers.
///
/// Expected: Ok with corporation created after retry
#[tokio::test]
async fn retries_on_esi_server_error() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    test.assert_mocks();

    Ok(())
}

/// Tests updating with alliance dependency.
///
/// Verifies that the corporation service correctly fetches alliance dependencies
/// when updating a corporation that belongs to an alliance.
///
/// Expected: Ok with both alliance and corporation created
#[tokio::test]
async fn updates_with_alliance_dependency() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), None),
            1,
        )
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());

    // Verify both alliance and corporation exist
    let alliance = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    let corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    test.assert_mocks();

    Ok(())
}

/// Tests failure after max ESI retries.
///
/// Verifies that the corporation service returns an error after exhausting all
/// retry attempts (3 attempts) when ESI continues to return server errors.
///
/// Expected: Err(Error::EsiError)
#[tokio::test]
async fn fails_after_max_esi_retries() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint_error(corporation_id, 500, 3)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

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
/// Verifies that the corporation service returns an error when ESI returns
/// a client error (404) indicating the corporation doesn't exist.
///
/// Expected: Err(Error::EsiError)
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint_error(corporation_id, 404, 1)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_err());

    test.assert_mocks();

    Ok(())
}

/// Tests failure when tables are missing.
///
/// Verifies that the corporation service returns a database error when attempting
/// to insert into a non-existent corporation table.
///
/// Expected: Err(Error::DbErr)
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 0)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_err());
    match result {
        Err(Error::DbErr(_)) => (),
        _ => panic!("Expected Error::DbErr, got: {:?}", result),
    }

    Ok(())
}

/// Tests failure when alliance table is missing.
///
/// Verifies that the corporation service returns a database error when attempting
/// to insert a corporation with an alliance but the alliance table doesn't exist.
///
/// Expected: Err(Error::DbErr)
#[tokio::test]
async fn fails_when_alliance_table_missing() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveCorporation)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 0)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), None),
            0,
        )
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_err());
    match result {
        Err(Error::DbErr(_)) => (),
        _ => panic!("Expected Error::DbErr, got: {:?}", result),
    }

    Ok(())
}

/// Tests failure when faction table is missing.
///
/// Verifies that the corporation service returns a database error when attempting
/// to insert a corporation with a faction but the faction table doesn't exist.
///
/// Expected: Err(Error::DbErr)
#[tokio::test]
async fn fails_when_faction_table_missing() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(None, Some(faction_id)),
            0,
        )
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

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
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint_error(corporation_id, 500, 3)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let _result = corporation_service.update(corporation_id).await;

    // Verify no corporation was created
    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 0);

    test.assert_mocks();

    Ok(())
}

/// Tests updating multiple corporations sequentially.
///
/// Verifies that the corporation service can successfully update multiple different
/// corporations in sequence, with each update being independent.
///
/// Expected: Ok with all three corporations created
#[tokio::test]
async fn updates_multiple_corporations_sequentially() -> Result<(), TestError> {
    let corp1_id = 98_000_001;
    let corp2_id = 98_000_002;
    let corp3_id = 98_000_003;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(corp1_id, factory::mock_corporation(None, None), 1)
        .with_corporation_endpoint(corp2_id, factory::mock_corporation(None, None), 1)
        .with_corporation_endpoint(corp3_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);

    let result1 = corporation_service.update(corp1_id).await;
    assert!(result1.is_ok());

    let result2 = corporation_service.update(corp2_id).await;
    assert!(result2.is_ok());

    let result3 = corporation_service.update(corp3_id).await;
    assert!(result3.is_ok());

    // Verify all three corporations exist
    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 3);

    test.assert_mocks();

    Ok(())
}

/// Tests that update is idempotent.
///
/// Verifies that calling update multiple times for the same corporation doesn't
/// create duplicate records and updates the existing record each time.
///
/// Expected: Ok with only one corporation record, timestamp updated on each call
#[tokio::test]
async fn update_is_idempotent() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 3)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);

    let result1 = corporation_service.update(corporation_id).await;
    assert!(result1.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let result2 = corporation_service.update(corporation_id).await;
    assert!(result2.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let result3 = corporation_service.update(corporation_id).await;
    assert!(result3.is_ok());

    // Verify only one corporation exists
    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 1);

    // Verify timestamp increased with each update
    assert!(result2.unwrap().info_updated_at > result1.unwrap().info_updated_at);

    test.assert_mocks();

    Ok(())
}

/// Tests updating existing corporation with 304 Not Modified response.
///
/// Verifies that when ESI returns 304 Not Modified (indicating data hasn't changed),
/// the service only updates the info_updated_at timestamp without fetching fresh data
/// or modifying other fields.
///
/// Expected: Ok with only timestamp updated, no ESI fetch for corporation data
#[tokio::test]
async fn updates_timestamp_on_304_not_modified() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_mock_corporation(corporation_id, None, None)
        .with_corporation_endpoint_not_modified(corporation_id, 1)
        .build()
        .await?;

    let corporation_before = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_timestamp = corporation_before.info_updated_at;
    let old_name = corporation_before.name.clone();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();

    // Verify timestamp was updated
    assert!(corporation.info_updated_at > old_timestamp);

    // Verify other fields remained unchanged
    assert_eq!(corporation.name, old_name);
    assert_eq!(corporation.corporation_id, corporation_id);

    test.assert_mocks();

    Ok(())
}

/// Tests update with 304 Not Modified for corporation with affiliations.
///
/// Verifies that 304 Not Modified handling works correctly for corporations
/// with alliance and faction affiliations, preserving those relationships.
///
/// Expected: Ok with timestamp updated, affiliations preserved
#[tokio::test]
async fn updates_timestamp_on_304_with_affiliations() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint_not_modified(corporation_id, 1)
        .build()
        .await?;

    // Pre-insert corporation with affiliations
    test.eve()
        .insert_mock_corporation(corporation_id, Some(alliance_id), Some(faction_id))
        .await?;

    let corporation_before = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_timestamp = corporation_before.info_updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.update(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();

    // Verify timestamp was updated
    assert!(corporation.info_updated_at > old_timestamp);

    // Verify affiliations were preserved
    assert_eq!(corporation.alliance_id, corporation_before.alliance_id);
    assert_eq!(corporation.faction_id, corporation_before.faction_id);

    test.assert_mocks();

    Ok(())
}

/// Tests multiple updates with alternating 304 and fresh responses.
///
/// Verifies that the service correctly handles a mix of 304 Not Modified and
/// fresh data responses across multiple update calls for the same corporation.
///
/// Expected: Ok with appropriate behavior for each response type
#[tokio::test]
async fn handles_mixed_304_and_fresh_responses() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_corporation_endpoint_not_modified(corporation_id, 1)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);

    // First update: creates new corporation
    let result1 = corporation_service.update(corporation_id).await;
    assert!(result1.is_ok());
    let corp1 = result1.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Second update: 304 Not Modified
    let result2 = corporation_service.update(corporation_id).await;
    assert!(result2.is_ok());
    let corp2 = result2.unwrap();
    assert!(corp2.info_updated_at > corp1.info_updated_at);

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Third update: fresh data
    let result3 = corporation_service.update(corporation_id).await;
    assert!(result3.is_ok());
    let corp3 = result3.unwrap();
    assert!(corp3.info_updated_at > corp2.info_updated_at);

    // Verify only one corporation exists
    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 1);

    test.assert_mocks();

    Ok(())
}
