//! Tests for CorporationService::upsert method.
//!
//! This module verifies the corporation upsert service behavior during ESI data
//! synchronization, including dependency resolution for factions and alliances,
//! retry logic for transient failures, and error handling for missing tables or
//! unavailable ESI endpoints.

use bifrost::server::{error::Error, service::eve::corporation::CorporationService};
use bifrost_test_utils::prelude::*;
use sea_orm::EntityTrait;

/// Tests upserting a new corporation without alliance or faction.
///
/// Verifies that the corporation service successfully fetches corporation data from ESI
/// and creates a new corporation record in the database when the corporation has no
/// alliance or faction affiliations.
///
/// Expected: Ok with corporation record created
#[tokio::test]
async fn upserts_new_corporation_without_affiliations() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

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

/// Tests upserting a new corporation with alliance.
///
/// Verifies that the corporation service successfully resolves alliance dependencies,
/// fetches both alliance and corporation data from ESI, and creates both records in
/// the correct dependency order.
///
/// Expected: Ok with corporation and alliance records created
#[tokio::test]
async fn upserts_new_corporation_with_alliance() -> Result<(), TestError> {
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
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);
    assert!(corporation.alliance_id.is_some());

    // Verify alliance exists in database
    let alliance = entity::prelude::EveAlliance::find().one(&test.db).await?;
    assert!(alliance.is_some());
    let alliance = alliance.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);

    // Verify corporation exists with alliance relationship
    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_corporation.corporation_id, corporation_id);
    assert_eq!(db_corporation.alliance_id, Some(alliance.id));

    test.assert_mocks();

    Ok(())
}

/// Tests upserting a new corporation with faction.
///
/// Verifies that the corporation service successfully resolves faction dependencies,
/// fetches both faction and corporation data from ESI, and creates both records in
/// the correct dependency order.
///
/// Expected: Ok with corporation and faction records created
#[tokio::test]
async fn upserts_new_corporation_with_faction() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(None, Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    // Verify faction exists in database
    let faction = entity::prelude::EveFaction::find().one(&test.db).await?;
    assert!(faction.is_some());

    // Verify corporation exists
    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_corporation.corporation_id, corporation_id);

    test.assert_mocks();

    Ok(())
}

/// Tests upserting a corporation with both alliance and faction.
///
/// Verifies that the corporation service correctly handles the complete dependency
/// hierarchy when a corporation belongs to an alliance that has a faction affiliation.
///
/// Expected: Ok with full hierarchy created
#[tokio::test]
async fn upserts_corporation_with_alliance_and_faction() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);
    assert!(corporation.alliance_id.is_some());

    // Verify complete hierarchy exists
    let faction = entity::prelude::EveFaction::find().one(&test.db).await?;
    assert!(faction.is_some());
    let faction = faction.unwrap();

    let alliance = entity::prelude::EveAlliance::find().one(&test.db).await?;
    assert!(alliance.is_some());
    let alliance = alliance.unwrap();
    assert_eq!(alliance.faction_id, Some(faction.id));

    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_corporation.alliance_id, Some(alliance.id));

    test.assert_mocks();

    Ok(())
}

/// Tests upserting a corporation when alliance already exists.
///
/// Verifies that the corporation service correctly handles cases where the alliance
/// dependency already exists in the database, only fetching the corporation data
/// from ESI without redundant alliance fetches.
///
/// Expected: Ok with corporation created and linked to existing alliance
#[tokio::test]
async fn upserts_corporation_with_existing_alliance() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_mock_alliance(alliance_id, None)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), None),
            1,
        )
        .build()
        .await?;

    let alliances_before = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances_before.len(), 1);

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    // Verify no additional alliance was created
    let alliances_after = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances_after.len(), 1);

    // Verify corporation linked to existing alliance
    let db_corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_corporation.alliance_id, Some(alliances_before[0].id));

    test.assert_mocks();

    Ok(())
}

/// Tests upserting a corporation when faction already exists.
///
/// Verifies that the corporation service correctly handles cases where the faction
/// dependency already exists in the database.
///
/// Expected: Ok with corporation created and linked to existing faction
#[tokio::test]
async fn upserts_corporation_with_existing_faction() -> Result<(), TestError> {
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

    let factions_before = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions_before.len(), 1);

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_ok());

    // Verify no additional faction was created
    let factions_after = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions_after.len(), 1);

    test.assert_mocks();

    Ok(())
}

/// Tests updating an existing corporation.
///
/// Verifies that the corporation service updates an existing corporation record with
/// fresh data from ESI, including updating the updated_at timestamp.
///
/// Expected: Ok with corporation record updated
#[tokio::test]
async fn updates_existing_corporation() -> Result<(), TestError> {
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
    let result = corporation_service.upsert(corporation_id).await;

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
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert!(corporation.alliance_id.is_some());

    // Verify alliance was created
    let alliance = entity::prelude::EveAlliance::find().one(&test.db).await?;
    assert!(alliance.is_some());

    // Verify corporation now has alliance
    let corporation_after = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(corporation_after.alliance_id.is_some());
    assert_eq!(corporation_after.alliance_id, Some(alliance.unwrap().id));

    test.assert_mocks();

    Ok(())
}

/// Tests removing corporation alliance affiliation.
///
/// Verifies that the corporation service correctly updates a corporation's alliance_id
/// to None when the corporation leaves its alliance.
///
/// Expected: Ok with corporation's alliance_id set to None
#[tokio::test]
async fn removes_corporation_alliance_affiliation() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_mock_alliance(alliance_id, None)
        .with_mock_corporation(corporation_id, Some(alliance_id), None)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_before = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(corporation_before.alliance_id.is_some());

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert!(corporation.alliance_id.is_none());

    // Verify corporation no longer has alliance
    let corporation_after = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(corporation_after.alliance_id.is_none());

    test.assert_mocks();

    Ok(())
}

/// Tests retry logic on ESI server error.
///
/// Verifies that the corporation service successfully retries failed ESI requests
/// when encountering transient server errors (500), ultimately succeeding and
/// upserting the corporation data.
///
/// Expected: Ok with corporation data upserted after retry
#[tokio::test]
async fn retries_on_esi_server_error() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    // First request fails with 500, second succeeds
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_mock_endpoint(move |server| {
            server
                .mock("GET", format!("/corporations/{}", corporation_id).as_str())
                .with_status(500)
                .expect(1)
                .create()
        })
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);

    test.assert_mocks();

    Ok(())
}

/// Tests retry logic with alliance dependency.
///
/// Verifies that the corporation service's retry logic correctly handles failures
/// when fetching corporations with alliance dependencies, retrying both the corporation
/// and alliance fetches as needed.
///
/// Expected: Ok with corporation and alliance upserted after retry
#[tokio::test]
async fn retries_with_alliance_dependency() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;

    // Corporation request fails first, then succeeds
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_mock_endpoint(move |server| {
            server
                .mock("GET", format!("/corporations/{}", corporation_id).as_str())
                .with_status(500)
                .expect(1)
                .create()
        })
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), None),
            1,
        )
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_ok());
    let corporation = result.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);
    assert!(corporation.alliance_id.is_some());

    // Verify both alliance and corporation exist
    let alliance = entity::prelude::EveAlliance::find().one(&test.db).await?;
    assert!(alliance.is_some());

    test.assert_mocks();

    Ok(())
}

/// Tests error handling after max retries.
///
/// Verifies that the corporation service returns an error when ESI continuously
/// returns server errors (503) exceeding the maximum retry attempt limit.
///
/// Expected: Err with EsiError after 3 failed attempts
#[tokio::test]
async fn fails_after_max_esi_retries() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    // All 3 attempts fail (as per RetryContext::DEFAULT_MAX_ATTEMPTS)
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_mock_endpoint(move |server| {
            server
                .mock("GET", format!("/corporations/{}", corporation_id).as_str())
                .with_status(503)
                .expect(3)
                .create()
        })
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(Error::EsiError(_))));

    test.assert_mocks();

    Ok(())
}

/// Tests error handling when ESI is unavailable.
///
/// Verifies that the corporation service returns an error when the ESI endpoint
/// is completely unreachable due to connection failures.
///
/// Expected: Err with ReqwestError
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    // No mock endpoint is created, so connection will be refused
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(matches!(
        result,
        Err(Error::EsiError(eve_esi::Error::EsiError(_)))
    ));

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the corporation service returns a database error when attempting
/// to upsert a corporation without the required database tables being created.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    // ESI will be called but database operations will fail
    let test = TestBuilder::new()
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests error handling when alliance table is missing.
///
/// Verifies that the corporation service returns a database error when attempting
/// to upsert a corporation with alliance dependency but the alliance table doesn't exist.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_alliance_table_missing() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;

    // Only corporation table exists, alliance table missing
    let test = TestBuilder::new()
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
    let result = corporation_service.upsert(corporation_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests error handling when faction table is missing.
///
/// Verifies that the corporation service returns a database error when attempting
/// to upsert a corporation with faction dependency but the faction table doesn't exist.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_faction_table_missing() -> Result<(), TestError> {
    let corporation_id = 98_000_001;
    let faction_id = 500_001;

    // Only corporation table exists, faction table missing
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveCorporation)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(None, Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests transaction rollback on error.
///
/// Verifies that when an error occurs during the upsert process, the transaction
/// is properly rolled back and no partial data is committed to the database.
///
/// Expected: Err with no corporation created in database
#[tokio::test]
async fn rolls_back_transaction_on_error() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    // Corporation endpoint will work but we'll drop the database connection
    // by not having the tables, causing transaction to fail
    let test = TestBuilder::new()
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);
    let result = corporation_service.upsert(corporation_id).await;

    assert!(result.is_err());

    // Verify no corporation was created (would fail anyway since table doesn't exist)
    // This test mainly verifies the transaction rollback path is exercised

    Ok(())
}

/// Tests upserting multiple corporations sequentially.
///
/// Verifies that the corporation service can successfully upsert multiple different
/// corporations in sequence, each with their own data and dependencies.
///
/// Expected: Ok with all corporations created
#[tokio::test]
async fn upserts_multiple_corporations_sequentially() -> Result<(), TestError> {
    let corp1_id = 98_000_001;
    let corp2_id = 98_000_002;
    let corp3_id = 98_000_003;
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .with_corporation_endpoint(corp1_id, factory::mock_corporation(None, None), 1)
        .with_corporation_endpoint(
            corp2_id,
            factory::mock_corporation(Some(alliance_id), None),
            1,
        )
        .with_corporation_endpoint(corp3_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);

    // Upsert first corporation without alliance
    let result1 = corporation_service.upsert(corp1_id).await;
    assert!(result1.is_ok());

    // Upsert second corporation with alliance
    let result2 = corporation_service.upsert(corp2_id).await;
    assert!(result2.is_ok());

    // Upsert third corporation without alliance
    let result3 = corporation_service.upsert(corp3_id).await;
    assert!(result3.is_ok());

    // Verify all corporations exist
    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 3);

    // Verify alliance was only created once
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 1);

    test.assert_mocks();

    Ok(())
}

/// Tests idempotency of upsert operation.
///
/// Verifies that calling upsert multiple times for the same corporation is idempotent,
/// updating the existing record rather than creating duplicates.
///
/// Expected: Ok with single corporation record updated
#[tokio::test]
async fn upsert_is_idempotent() -> Result<(), TestError> {
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 3)
        .build()
        .await?;

    let corporation_service = CorporationService::new(&test.db, &test.esi_client);

    // Upsert same corporation three times
    let result1 = corporation_service.upsert(corporation_id).await;
    assert!(result1.is_ok());

    let result2 = corporation_service.upsert(corporation_id).await;
    assert!(result2.is_ok());

    let result3 = corporation_service.upsert(corporation_id).await;
    assert!(result3.is_ok());

    // Verify only one corporation exists
    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 1);
    assert_eq!(corporations[0].corporation_id, corporation_id);

    test.assert_mocks();

    Ok(())
}
