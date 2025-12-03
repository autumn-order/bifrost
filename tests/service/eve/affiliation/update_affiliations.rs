//! Tests for AffiliationService::update_affiliations method.
//!
//! This module verifies the affiliation update service behavior during bulk character
//! and corporation affiliation updates from ESI, including dependency resolution,
//! transaction handling, retry logic, input validation, and error handling.

use bifrost::server::{
    error::Error, service::eve::affiliation::AffiliationService,
    util::eve::ESI_AFFILIATION_REQUEST_LIMIT,
};
use bifrost_test_utils::prelude::*;
use sea_orm::EntityTrait;

/// Tests updating affiliations with a single character.
///
/// Verifies that the affiliation service successfully fetches character affiliation
/// data from ESI, creates all necessary dependencies (faction, alliance, corporation,
/// character), and updates their relationships in a single transaction.
///
/// Expected: Ok with all entities and relationships created
#[tokio::test]
async fn updates_single_character_affiliation() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                character_id,
                corporation_id,
                Some(alliance_id),
                Some(faction_id),
            )],
            1,
        )
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), Some(faction_id)),
            1,
        )
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, Some(alliance_id), Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_ok());

    // Verify all entities were created
    let faction = entity::prelude::EveFaction::find().one(&test.db).await?;
    assert!(faction.is_some());
    let faction = faction.unwrap();
    assert_eq!(faction.faction_id, faction_id);

    let alliance = entity::prelude::EveAlliance::find().one(&test.db).await?;
    assert!(alliance.is_some());
    let alliance = alliance.unwrap();
    assert_eq!(alliance.alliance_id, alliance_id);
    assert_eq!(alliance.faction_id, Some(faction.id));

    let corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?;
    assert!(corporation.is_some());
    let corporation = corporation.unwrap();
    assert_eq!(corporation.corporation_id, corporation_id);
    assert_eq!(corporation.alliance_id, Some(alliance.id));

    let character = entity::prelude::EveCharacter::find().one(&test.db).await?;
    assert!(character.is_some());
    let character = character.unwrap();
    assert_eq!(character.character_id, character_id);
    assert_eq!(character.corporation_id, corporation.id);
    assert_eq!(character.faction_id, Some(faction.id));

    test.assert_mocks();

    Ok(())
}

/// Tests updating affiliations with multiple characters.
///
/// Verifies that the affiliation service correctly handles bulk updates with multiple
/// characters, creating all unique entities and establishing proper relationships for
/// each character.
///
/// Expected: Ok with all characters and their dependencies created
#[tokio::test]
async fn updates_multiple_character_affiliations() -> Result<(), TestError> {
    let char1_id = 95_000_001;
    let char2_id = 95_000_002;
    let char3_id = 95_000_003;
    let corp1_id = 98_000_001;
    let corp2_id = 98_000_002;
    let alliance_id = 99_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(
            vec![
                factory::mock_character_affiliation(char1_id, corp1_id, Some(alliance_id), None),
                factory::mock_character_affiliation(char2_id, corp1_id, Some(alliance_id), None),
                factory::mock_character_affiliation(char3_id, corp2_id, None, None),
            ],
            1,
        )
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(None), 1)
        .with_corporation_endpoint(
            corp1_id,
            factory::mock_corporation(Some(alliance_id), None),
            1,
        )
        .with_corporation_endpoint(corp2_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            char1_id,
            factory::mock_character(corp1_id, Some(alliance_id), None),
            1,
        )
        .with_character_endpoint(
            char2_id,
            factory::mock_character(corp1_id, Some(alliance_id), None),
            1,
        )
        .with_character_endpoint(char3_id, factory::mock_character(corp2_id, None, None), 1)
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![char1_id, char2_id, char3_id])
        .await;

    assert!(result.is_ok());

    // Verify correct number of entities created
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 1);

    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 2);

    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), 3);

    test.assert_mocks();

    Ok(())
}

/// Tests updating affiliations when entities already exist.
///
/// Verifies that the affiliation service correctly handles cases where factions,
/// alliances, corporations, or characters already exist in the database, only
/// updating relationships rather than creating duplicate entities.
///
/// Expected: Ok with relationships updated and no duplicate entities
#[tokio::test]
async fn updates_affiliations_with_existing_entities() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_faction(faction_id)
        .with_mock_alliance(alliance_id, Some(faction_id))
        .with_mock_corporation(corporation_id, Some(alliance_id), Some(faction_id))
        .with_mock_character(
            character_id,
            corporation_id,
            Some(alliance_id),
            Some(faction_id),
        )
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                character_id,
                corporation_id,
                Some(alliance_id),
                Some(faction_id),
            )],
            1,
        )
        .build()
        .await?;

    // Update character to have no corporation initially
    let character = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_corp_id = character.corporation_id;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_ok());

    // Verify no duplicate entities created
    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions.len(), 1);

    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 1);

    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 1);

    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), 1);

    // Verify relationships still correct
    let character = characters[0].clone();
    assert_eq!(character.corporation_id, old_corp_id);

    test.assert_mocks();

    Ok(())
}

/// Tests updating corporation affiliation to alliance.
///
/// Verifies that the affiliation service correctly updates a corporation's alliance_id
/// when processing character affiliations, handling the corporation-to-alliance
/// relationship.
///
/// Expected: Ok with corporation's alliance_id updated
#[tokio::test]
async fn updates_corporation_alliance_affiliation() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;
    let old_alliance_id = 99_000_001;
    let new_alliance_id = 99_000_002;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_alliance(old_alliance_id, None)
        .with_mock_corporation(corporation_id, Some(old_alliance_id), None)
        .with_mock_character(character_id, corporation_id, Some(old_alliance_id), None)
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                character_id,
                corporation_id,
                Some(new_alliance_id),
                None,
            )],
            1,
        )
        .with_alliance_endpoint(new_alliance_id, factory::mock_alliance(None), 1)
        .build()
        .await?;

    let corp_before = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_alliance_db_id = corp_before.alliance_id;
    assert!(old_alliance_db_id.is_some());

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_ok());

    // Verify corporation's alliance was updated
    let corp_after = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(corp_after.alliance_id.is_some());
    assert_ne!(corp_after.alliance_id, old_alliance_db_id);

    // Verify both alliances exist
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 2);

    test.assert_mocks();

    Ok(())
}

/// Tests updating character affiliation to different corporation.
///
/// Verifies that the affiliation service correctly updates a character's corporation_id
/// when their corporate membership changes, reflecting the new affiliation in the database.
///
/// Expected: Ok with character's corporation_id updated
#[tokio::test]
async fn updates_character_corporation_affiliation() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let old_corp_id = 98_000_001;
    let new_corp_id = 98_000_002;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_corporation(old_corp_id, None, None)
        .with_mock_character(character_id, old_corp_id, None, None)
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                character_id,
                new_corp_id,
                None,
                None,
            )],
            1,
        )
        .with_corporation_endpoint(new_corp_id, factory::mock_corporation(None, None), 1)
        .build()
        .await?;

    let char_before = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_corp_db_id = char_before.corporation_id;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_ok());

    // Verify character's corporation was updated
    let char_after = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_ne!(char_after.corporation_id, old_corp_db_id);

    // Verify both corporations exist
    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 2);

    test.assert_mocks();

    Ok(())
}

/// Tests updating character faction affiliation.
///
/// Verifies that the affiliation service correctly updates a character's faction_id
/// when their factional warfare allegiance changes, including setting it to None
/// when they leave factional warfare.
///
/// Expected: Ok with character's faction_id updated or cleared
#[tokio::test]
async fn updates_character_faction_affiliation() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_corporation(corporation_id, None, None)
        .with_mock_character(character_id, corporation_id, None, None)
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                character_id,
                corporation_id,
                None,
                Some(faction_id),
            )],
            1,
        )
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .build()
        .await?;

    let char_before = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(char_before.faction_id.is_none());

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_ok());

    // Verify character's faction was set
    let char_after = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(char_after.faction_id.is_some());

    test.assert_mocks();

    Ok(())
}

/// Tests handling empty character ID list.
///
/// Verifies that the affiliation service gracefully handles empty input,
/// returning Ok without making any ESI calls or database operations.
///
/// Expected: Ok with no operations performed
#[tokio::test]
async fn handles_empty_character_list() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(vec![], 0)
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service.update_affiliations(vec![]).await;

    assert!(result.is_ok());

    test.assert_mocks();

    Ok(())
}

/// Tests truncating character IDs exceeding ESI limit.
///
/// Verifies that the affiliation service automatically truncates the character ID list
/// to ESI's bulk affiliation request limit (1000), preventing request failures due to
/// oversized payloads.
///
/// Expected: Ok with only first 1000 characters processed
#[tokio::test]
async fn truncates_character_ids_exceeding_esi_limit() -> Result<(), TestError> {
    let character_ids: Vec<i64> = (95_000_001..=95_001_500)
        .collect::<Vec<_>>()
        .into_iter()
        .take(1500)
        .collect();
    let truncated_ids: Vec<i64> = character_ids
        .iter()
        .take(ESI_AFFILIATION_REQUEST_LIMIT)
        .copied()
        .collect();

    let affiliations: Vec<_> = truncated_ids
        .iter()
        .map(|&id| factory::mock_character_affiliation(id, 98_000_001, None, None))
        .collect();

    let mut test_builder = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(affiliations, 1)
        .with_corporation_endpoint(98_000_001, factory::mock_corporation(None, None), 1);

    // Add character endpoints for the first ESI_AFFILIATION_REQUEST_LIMIT characters
    for &id in truncated_ids.iter() {
        test_builder = test_builder.with_character_endpoint(
            id,
            factory::mock_character(98_000_001, None, None),
            1,
        );
    }

    let test = test_builder.build().await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service.update_affiliations(character_ids).await;

    assert!(result.is_ok());

    // Verify only ESI_AFFILIATION_REQUEST_LIMIT characters were processed
    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), ESI_AFFILIATION_REQUEST_LIMIT);

    test.assert_mocks();

    Ok(())
}

/// Tests filtering invalid character IDs.
///
/// Verifies that the affiliation service automatically filters out invalid character IDs
/// that fall outside EVE's valid character ID ranges, preventing ESI request failures
/// caused by malformed IDs.
///
/// Expected: Ok with only valid character IDs processed
#[tokio::test]
async fn filters_invalid_character_ids() -> Result<(), TestError> {
    let valid_char_id = 95_000_001;
    let invalid_char_ids = vec![
        1_000_000,     // Too low
        89_999_999,    // Below first range
        98_000_000,    // Between first and second range
        2_099_999_999, // Just before third range (actually valid per the range)
        2_112_000_000, // Start of fourth range (valid)
        2_130_000_000, // Above all ranges
    ];

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                valid_char_id,
                98_000_001,
                None,
                None,
            )],
            1,
        )
        .with_corporation_endpoint(98_000_001, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            valid_char_id,
            factory::mock_character(98_000_001, None, None),
            1,
        )
        .build()
        .await?;

    let mut character_ids = invalid_char_ids.clone();
    character_ids.push(valid_char_id);

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service.update_affiliations(character_ids).await;

    assert!(result.is_ok());

    // Verify only the valid character was processed
    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].character_id, valid_char_id);

    test.assert_mocks();

    Ok(())
}

/// Tests handling all invalid character IDs.
///
/// Verifies that the affiliation service correctly handles the case where all provided
/// character IDs are invalid, returning Ok without making any ESI calls.
///
/// Expected: Ok with no operations performed
#[tokio::test]
async fn handles_all_invalid_character_ids() -> Result<(), TestError> {
    let invalid_char_ids = vec![
        1_000_000,     // Too low
        89_999_999,    // Below valid range
        98_000_000,    // Between ranges
        2_130_000_000, // Above valid range
    ];

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(vec![], 0)
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(invalid_char_ids)
        .await;

    assert!(result.is_ok());

    // Verify no entities were created
    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), 0);

    test.assert_mocks();

    Ok(())
}

/// Tests retry logic on ESI server error.
///
/// Verifies that the affiliation service successfully retries failed ESI requests
/// when encountering transient server errors (500), ultimately succeeding after retry.
///
/// Expected: Ok with affiliation data updated after retry
#[tokio::test]
async fn retries_on_esi_server_error() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    // First request fails with 500, second succeeds
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_endpoint(|server| {
            server
                .mock("POST", "/characters/affiliation")
                .with_status(500)
                .expect(1)
                .create()
        })
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                character_id,
                corporation_id,
                None,
                None,
            )],
            1,
        )
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_ok());

    // Verify entities were created
    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), 1);

    test.assert_mocks();

    Ok(())
}

/// Tests error handling after max retries.
///
/// Verifies that the affiliation service returns an error when ESI continuously
/// returns server errors (503) exceeding the maximum retry attempt limit.
///
/// Expected: Err with EsiError after 3 failed attempts
#[tokio::test]
async fn fails_after_max_esi_retries() -> Result<(), TestError> {
    let character_id = 95_000_001;

    // All 3 attempts fail (as per RetryContext::DEFAULT_MAX_ATTEMPTS)
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_endpoint(|server| {
            server
                .mock("POST", "/characters/affiliation")
                .with_status(503)
                .expect(3)
                .create()
        })
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_err());
    assert!(matches!(result, Err(Error::EsiError(_))));

    test.assert_mocks();

    Ok(())
}

/// Tests error handling when ESI is unavailable.
///
/// Verifies that the affiliation service returns an error when the ESI endpoint
/// is completely unreachable due to connection failures.
///
/// Expected: Err with ReqwestError
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let character_id = 95_000_001;

    // No mock endpoint is created, so connection will be refused
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(matches!(
        result,
        Err(Error::EsiError(eve_esi::Error::ReqwestError(_)))
    ));

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the affiliation service returns a database error when attempting
/// to update affiliations without the required database tables being created.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    // ESI will be called but database operations will fail
    let test = TestBuilder::new()
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                character_id,
                corporation_id,
                None,
                None,
            )],
            1,
        )
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests handling empty affiliation response from ESI.
///
/// Verifies that the affiliation service gracefully handles cases where ESI returns
/// an empty affiliation list (no matches for the requested character IDs), returning
/// Ok without attempting any database operations.
///
/// Expected: Ok with no database changes
#[tokio::test]
async fn handles_empty_affiliation_response() -> Result<(), TestError> {
    let character_id = 95_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(vec![], 1)
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_ok());

    // Verify no entities were created
    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), 0);

    test.assert_mocks();

    Ok(())
}

/// Tests updating affiliations with no alliance.
///
/// Verifies that the affiliation service correctly handles characters and corporations
/// that are not members of any alliance, setting alliance_id fields to None.
///
/// Expected: Ok with entities created and alliance_id set to None
#[tokio::test]
async fn updates_affiliations_with_no_alliance() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                character_id,
                corporation_id,
                None,
                None,
            )],
            1,
        )
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_ok());

    // Verify corporation has no alliance
    let corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(corporation.alliance_id.is_none());

    // Verify no alliances were created
    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 0);

    test.assert_mocks();

    Ok(())
}

/// Tests updating affiliations with no faction.
///
/// Verifies that the affiliation service correctly handles characters that are not
/// enrolled in factional warfare, setting faction_id to None.
///
/// Expected: Ok with character created and faction_id set to None
#[tokio::test]
async fn updates_affiliations_with_no_faction() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                character_id,
                corporation_id,
                None,
                None,
            )],
            1,
        )
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_ok());

    // Verify character has no faction
    let character = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(character.faction_id.is_none());

    // Verify no factions were created
    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions.len(), 0);

    test.assert_mocks();

    Ok(())
}

/// Tests complete affiliation hierarchy with faction, alliance, corporation, and character.
///
/// Verifies that the affiliation service correctly handles the complete entity hierarchy
/// when a character belongs to a corporation in an alliance that belongs to a faction,
/// and the character is also enrolled in factional warfare.
///
/// Expected: Ok with full hierarchy created and all relationships established
#[tokio::test]
async fn updates_complete_affiliation_hierarchy() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(
            vec![factory::mock_character_affiliation(
                character_id,
                corporation_id,
                Some(alliance_id),
                Some(faction_id),
            )],
            1,
        )
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), Some(faction_id)),
            1,
        )
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, Some(alliance_id), Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![character_id])
        .await;

    assert!(result.is_ok());

    // Verify complete hierarchy
    let faction = entity::prelude::EveFaction::find().one(&test.db).await?;
    assert!(faction.is_some());
    let faction = faction.unwrap();

    let alliance = entity::prelude::EveAlliance::find().one(&test.db).await?;
    assert!(alliance.is_some());
    let alliance = alliance.unwrap();
    assert_eq!(alliance.faction_id, Some(faction.id));

    let corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?;
    assert!(corporation.is_some());
    let corporation = corporation.unwrap();
    assert_eq!(corporation.alliance_id, Some(alliance.id));

    let character = entity::prelude::EveCharacter::find().one(&test.db).await?;
    assert!(character.is_some());
    let character = character.unwrap();
    assert_eq!(character.corporation_id, corporation.id);
    assert_eq!(character.faction_id, Some(faction.id));

    test.assert_mocks();

    Ok(())
}

/// Tests deduplication of entity IDs in affiliation data.
///
/// Verifies that the affiliation service correctly deduplicates entity IDs when multiple
/// characters share the same corporation, alliance, or faction, ensuring entities are
/// only fetched and created once.
///
/// Expected: Ok with deduplicated entity fetches and single instances created
#[tokio::test]
async fn deduplicates_entity_ids_in_affiliation_data() -> Result<(), TestError> {
    let char1_id = 95_000_001;
    let char2_id = 95_000_002;
    let char3_id = 95_000_003;
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    // All three characters share same corporation, alliance, and faction
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_character_affiliation_endpoint(
            vec![
                factory::mock_character_affiliation(
                    char1_id,
                    corporation_id,
                    Some(alliance_id),
                    Some(faction_id),
                ),
                factory::mock_character_affiliation(
                    char2_id,
                    corporation_id,
                    Some(alliance_id),
                    Some(faction_id),
                ),
                factory::mock_character_affiliation(
                    char3_id,
                    corporation_id,
                    Some(alliance_id),
                    Some(faction_id),
                ),
            ],
            1,
        )
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_alliance_endpoint(alliance_id, factory::mock_alliance(Some(faction_id)), 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(Some(alliance_id), Some(faction_id)),
            1,
        )
        .with_character_endpoint(
            char1_id,
            factory::mock_character(corporation_id, Some(alliance_id), Some(faction_id)),
            1,
        )
        .with_character_endpoint(
            char2_id,
            factory::mock_character(corporation_id, Some(alliance_id), Some(faction_id)),
            1,
        )
        .with_character_endpoint(
            char3_id,
            factory::mock_character(corporation_id, Some(alliance_id), Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let affiliation_service = AffiliationService::new(&test.db, &test.esi_client);
    let result = affiliation_service
        .update_affiliations(vec![char1_id, char2_id, char3_id])
        .await;

    assert!(result.is_ok());

    // Verify only one of each entity was created despite 3 characters
    let factions = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions.len(), 1);

    let alliances = entity::prelude::EveAlliance::find().all(&test.db).await?;
    assert_eq!(alliances.len(), 1);

    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 1);

    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), 3);

    // Each endpoint should only be called once due to deduplication
    test.assert_mocks();

    Ok(())
}
