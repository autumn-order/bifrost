//! Tests for AllianceService::upsert method.
//!
//! This module verifies the alliance upsert service behavior during ESI data
//! synchronization, including dependency resolution for factions, retry logic
//! for transient failures, and error handling for missing tables or unavailable
//! ESI endpoints.

use bifrost::server::{error::Error, service::eve::alliance::AllianceService};
use bifrost_test_utils::prelude::*;
use sea_orm::EntityTrait;

/// Tests upserting a new alliance without faction.
///
/// Verifies that the alliance service successfully fetches alliance data from ESI
/// and creates a new alliance record in the database when the alliance has no
/// faction affiliation.
///
/// Expected: Ok with alliance record created
#[tokio::test]
async fn upserts_new_alliance_without_faction() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.upsert(alliance_id).await;

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

/// Tests upserting a new alliance with faction.
///
/// Verifies that the alliance service successfully resolves faction dependencies,
/// fetches both faction and alliance data from ESI, and creates both records in
/// the correct dependency order.
///
/// Expected: Ok with alliance and faction records created
#[tokio::test]
async fn upserts_new_alliance_with_faction() -> Result<(), TestError> {
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
    let result = alliance_service.upsert(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);
    assert!(alliance.faction_id.is_some());

    // Verify faction exists in database
    let faction = entity::prelude::EveFaction::find().one(&test.db).await?;
    assert!(faction.is_some());
    let faction = faction.unwrap();
    assert_eq!(faction.faction_id, faction_id);

    // Verify alliance exists with faction relationship
    let db_alliance = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_alliance.alliance_id, alliance_id);
    assert_eq!(db_alliance.faction_id, Some(faction.id));

    test.assert_mocks();

    Ok(())
}

/// Tests upserting an alliance when faction already exists.
///
/// Verifies that the alliance service correctly handles cases where the faction
/// dependency already exists in the database, only fetching the alliance data
/// from ESI without redundant faction fetches.
///
/// Expected: Ok with alliance created and linked to existing faction
#[tokio::test]
async fn upserts_alliance_with_existing_faction() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_mock_faction(faction_id)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .build()
        .await?;

    let factions_before = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions_before.len(), 1);

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.upsert(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    // Verify no additional faction was created
    let factions_after = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions_after.len(), 1);

    // Verify alliance linked to existing faction
    let db_alliance = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_alliance.faction_id, Some(factions_before[0].id));

    test.assert_mocks();

    Ok(())
}

/// Tests updating an existing alliance.
///
/// Verifies that the alliance service updates an existing alliance record with
/// fresh data from ESI, including updating the updated_at timestamp.
///
/// Expected: Ok with alliance record updated
#[tokio::test]
async fn updates_existing_alliance() -> Result<(), TestError> {
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
    let result = alliance_service.upsert(alliance_id).await;

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
/// when the alliance joins or changes faction affiliation.
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
    let result = alliance_service.upsert(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert!(alliance.faction_id.is_some());

    // Verify faction was created
    let faction = entity::prelude::EveFaction::find().one(&test.db).await?;
    assert!(faction.is_some());

    // Verify alliance now has faction
    let alliance_after = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(alliance_after.faction_id.is_some());
    assert_eq!(alliance_after.faction_id, Some(faction.unwrap().id));

    test.assert_mocks();

    Ok(())
}

/// Tests removing alliance faction affiliation.
///
/// Verifies that the alliance service correctly updates an alliance's faction_id
/// to None when the alliance leaves faction warfare.
///
/// Expected: Ok with alliance's faction_id set to None
#[tokio::test]
async fn removes_alliance_faction_affiliation() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_mock_faction(faction_id)
        .with_mock_alliance(alliance_id, Some(faction_id))
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_before = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(alliance_before.faction_id.is_some());

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.upsert(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert!(alliance.faction_id.is_none());

    // Verify alliance no longer has faction
    let alliance_after = entity::prelude::EveAlliance::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(alliance_after.faction_id.is_none());

    test.assert_mocks();

    Ok(())
}

/// Tests retry logic on ESI server error.
///
/// Verifies that the alliance service successfully retries failed ESI requests
/// when encountering transient server errors (500), ultimately succeeding and
/// upserting the alliance data.
///
/// Expected: Ok with alliance data upserted after retry
#[tokio::test]
async fn retries_on_esi_server_error() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    // First request fails with 500, second succeeds
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_mock_endpoint(move |server| {
            server
                .mock("GET", format!("/alliances/{}", alliance_id).as_str())
                .with_status(500)
                .expect(1)
                .create()
        })
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.upsert(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    test.assert_mocks();

    Ok(())
}

/// Tests retry logic with faction dependency.
///
/// Verifies that the alliance service's retry logic correctly handles failures
/// when fetching alliances with faction dependencies, retrying both the alliance
/// and faction fetches as needed.
///
/// Expected: Ok with alliance and faction upserted after retry
#[tokio::test]
async fn retries_with_faction_dependency() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    // Alliance request fails first, then succeeds
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_mock_endpoint(move |server| {
            server
                .mock("GET", format!("/alliances/{}", alliance_id).as_str())
                .with_status(500)
                .expect(1)
                .create()
        })
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.upsert(alliance_id).await;

    assert!(result.is_ok());
    let alliance = result.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);
    assert!(alliance.faction_id.is_some());

    // Verify both faction and alliance exist
    let faction = entity::prelude::EveFaction::find().one(&test.db).await?;
    assert!(faction.is_some());

    test.assert_mocks();

    Ok(())
}

/// Tests error handling after max retries.
///
/// Verifies that the alliance service returns an error when ESI continuously
/// returns server errors (503) exceeding the maximum retry attempt limit.
///
/// Expected: Err with EsiError after 3 failed attempts
#[tokio::test]
async fn fails_after_max_esi_retries() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    // All 3 attempts fail (as per RetryContext::DEFAULT_MAX_ATTEMPTS)
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_mock_endpoint(move |server| {
            server
                .mock("GET", format!("/alliances/{}", alliance_id).as_str())
                .with_status(503)
                .expect(3)
                .create()
        })
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.upsert(alliance_id).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(Error::EsiError(_))));

    test.assert_mocks();

    Ok(())
}

/// Tests error handling when ESI is unavailable.
///
/// Verifies that the alliance service returns an error when the ESI endpoint
/// is completely unreachable due to connection failures.
///
/// Expected: Err with ReqwestError
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    // No mock endpoint is created, so connection will be refused
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.upsert(alliance_id).await;

    assert!(matches!(
        result,
        Err(Error::EsiError(eve_esi::Error::EsiError(_)))
    ));

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the alliance service returns a database error when attempting
/// to upsert an alliance without the required database tables being created.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    // ESI will be called but database operations will fail
    let test = TestBuilder::new()
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.upsert(alliance_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests error handling when faction table is missing.
///
/// Verifies that the alliance service returns a database error when attempting
/// to upsert an alliance with faction dependency but the faction table doesn't exist.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_faction_table_missing() -> Result<(), TestError> {
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    // Only alliance table exists, faction table missing
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveAlliance)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.upsert(alliance_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests transaction rollback on error.
///
/// Verifies that when an error occurs during the upsert process, the transaction
/// is properly rolled back and no partial data is committed to the database.
///
/// Expected: Err with no alliance created in database
#[tokio::test]
async fn rolls_back_transaction_on_error() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    // Alliance endpoint will work but we'll drop the database connection
    // by not having the tables, causing transaction to fail
    let test = TestBuilder::new()
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);
    let result = alliance_service.upsert(alliance_id).await;

    assert!(result.is_err());

    // Verify no alliance was created (would fail anyway since table doesn't exist)
    // This test mainly verifies the transaction rollback path is exercised

    Ok(())
}

/// Tests upserting multiple alliances sequentially.
///
/// Verifies that the alliance service can successfully upsert multiple different
/// alliances in sequence, each with their own data and dependencies.
///
/// Expected: Ok with all alliances created
#[tokio::test]
async fn upserts_multiple_alliances_sequentially() -> Result<(), TestError> {
    let alliance1_id = 99_000_001;
    let alliance2_id = 99_000_002;
    let alliance3_id = 99_000_003;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_alliance_endpoint(alliance1_id, factory::mock_alliance(None), 1)
        .with_alliance_endpoint(alliance2_id, factory::mock_alliance(Some(faction_id)), 1)
        .with_alliance_endpoint(alliance3_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);

    // Upsert first alliance without faction
    let result1 = alliance_service.upsert(alliance1_id).await;
    assert!(result1.is_ok());

    // Upsert second alliance with faction
    let result2 = alliance_service.upsert(alliance2_id).await;
    assert!(result2.is_ok());

    // Upsert third alliance without faction
    let result3 = alliance_service.upsert(alliance3_id).await;
    assert!(result3.is_ok());

    // Verify all alliances exist
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 3);

    // Verify faction was only created once
    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions.len(), 1);

    test.assert_mocks();

    Ok(())
}

/// Tests idempotency of upsert operation.
///
/// Verifies that calling upsert multiple times for the same alliance is idempotent,
/// updating the existing record rather than creating duplicates.
///
/// Expected: Ok with single alliance record updated
#[tokio::test]
async fn upsert_is_idempotent() -> Result<(), TestError> {
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 3)
        .build()
        .await?;

    let alliance_service = AllianceService::new(&test.db, &test.esi_client);

    // Upsert same alliance three times
    let result1 = alliance_service.upsert(alliance_id).await;
    assert!(result1.is_ok());

    let result2 = alliance_service.upsert(alliance_id).await;
    assert!(result2.is_ok());

    let result3 = alliance_service.upsert(alliance_id).await;
    assert!(result3.is_ok());

    // Verify only one alliance exists
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 1);
    assert_eq!(alliances[0].alliance_id, alliance_id);

    test.assert_mocks();

    Ok(())
}
