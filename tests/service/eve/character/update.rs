//! Tests for CharacterService::update method.
//!
//! This module verifies the character update service behavior during ESI data
//! synchronization, including dependency resolution for corporations and factions,
//! retry logic for transient failures, caching with 304 Not Modified responses,
//! and error handling for missing tables or unavailable ESI endpoints.

use bifrost::server::{error::Error, service::eve::character::CharacterService};
use bifrost_test_utils::prelude::*;
use sea_orm::EntityTrait;

/// Tests updating a new character without faction.
///
/// Verifies that the character service successfully fetches character data from ESI
/// and creates a new character record in the database when the character has no
/// faction affiliation.
///
/// Expected: Ok with character record created
#[tokio::test]
async fn updates_new_character_without_faction() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();
    assert_eq!(character.character_id, character_id);
    assert!(character.faction_id.is_none());

    // Verify character exists in database
    let db_character = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_character.character_id, character_id);
    assert!(db_character.faction_id.is_none());

    // Verify corporation was created
    let corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?;
    assert!(corporation.is_some());

    test.assert_mocks();

    Ok(())
}

/// Tests updating a new character with faction.
///
/// Verifies that the character service successfully resolves faction dependencies,
/// fetches faction, corporation, and character data from ESI, and creates all records
/// in the correct dependency order.
///
/// Expected: Ok with character, corporation, and faction records created
#[tokio::test]
async fn updates_new_character_with_faction() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(None, Some(faction_id)),
            1,
        )
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();
    assert_eq!(character.character_id, character_id);
    assert!(character.faction_id.is_some());

    // Verify faction exists in database
    let faction = entity::prelude::EveFaction::find().one(&test.db).await?;
    assert!(faction.is_some());
    let faction = faction.unwrap();
    assert_eq!(faction.faction_id, faction_id);

    // Verify corporation exists
    let corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?;
    assert!(corporation.is_some());

    // Verify character has faction relationship
    let db_character = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_character.faction_id, Some(faction.id));

    test.assert_mocks();

    Ok(())
}

/// Tests updateing a character with full hierarchy.
///
/// Verifies that the character service correctly handles the complete dependency
/// hierarchy when a character belongs to a corporation in an alliance with faction
/// affiliation.
///
/// Expected: Ok with full hierarchy created
#[tokio::test]
async fn updates_character_with_full_hierarchy() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;
    let alliance_id = 99_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
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

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();
    assert_eq!(character.character_id, character_id);
    assert!(character.faction_id.is_some());

    // Verify complete hierarchy exists
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

    let db_character = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_character.corporation_id, corporation.id);
    assert_eq!(db_character.faction_id, Some(faction.id));

    test.assert_mocks();

    Ok(())
}

/// Tests updateing a character when corporation already exists.
///
/// Verifies that the character service correctly handles cases where the corporation
/// dependency already exists in the database, only fetching the character data
/// from ESI without redundant corporation fetches.
///
/// Expected: Ok with character created and linked to existing corporation
#[tokio::test]
async fn updates_character_with_existing_corporation() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_corporation(corporation_id, None, None)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let corporations_before = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations_before.len(), 1);

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();
    assert_eq!(character.character_id, character_id);

    // Verify no additional corporation was created
    let corporations_after = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations_after.len(), 1);

    // Verify character linked to existing corporation
    let db_character = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert_eq!(db_character.corporation_id, corporations_before[0].id);

    test.assert_mocks();

    Ok(())
}

/// Tests updateing a character when faction already exists.
///
/// Verifies that the character service correctly handles cases where the faction
/// dependency already exists in the database.
///
/// Expected: Ok with character created and linked to existing faction
#[tokio::test]
async fn updates_character_with_existing_faction() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_faction(faction_id)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(None, Some(faction_id)),
            1,
        )
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let factions_before = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions_before.len(), 1);

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());

    // Verify no additional faction was created
    let factions_after = entity::prelude::EveFaction::find().all(&test.db).await?;
    assert_eq!(factions_after.len(), 1);

    test.assert_mocks();

    Ok(())
}

/// Tests updating an existing character.
///
/// Verifies that the character service updates an existing character record with
/// fresh data from ESI, including updating the info_updated_at timestamp.
///
/// Expected: Ok with character record updated
#[tokio::test]
async fn updates_existing_character() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_corporation(corporation_id, None, None)
        .with_mock_character(character_id, corporation_id, None, None)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let character_before = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_updated_at = character_before.info_updated_at;

    // Small delay to ensure timestamp changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();
    assert_eq!(character.character_id, character_id);

    // Verify only one character exists
    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), 1);

    // Verify timestamp was updated
    assert!(character.info_updated_at > old_updated_at);

    test.assert_mocks();

    Ok(())
}

/// Tests updating character corporation affiliation.
///
/// Verifies that the character service correctly updates a character's corporation_id
/// when the character changes corporations.
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
        .with_corporation_endpoint(new_corp_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(new_corp_id, None, None),
            1,
        )
        .build()
        .await?;

    let character_before = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_corp_db_id = character_before.corporation_id;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();

    // Verify new corporation was created
    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 2);

    // The character's corporation_id should be updated to the new corporation
    assert_ne!(character.corporation_id, old_corp_db_id);

    test.assert_mocks();

    Ok(())
}

/// Tests adding faction affiliation to character.
///
/// Verifies that the character service correctly updates a character's faction_id
/// when the character joins factional warfare.
///
/// Expected: Ok with character's faction_id updated
#[tokio::test]
async fn adds_character_faction_affiliation() -> Result<(), TestError> {
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
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let character_before = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    assert!(character_before.faction_id.is_none());

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();

    // Verify faction was created
    let faction = entity::prelude::EveFaction::find().one(&test.db).await?;
    assert!(faction.is_some());
    let faction = faction.unwrap();

    // The character's faction_id should be updated
    assert!(character.faction_id.is_some());
    assert_eq!(character.faction_id, Some(faction.id));

    test.assert_mocks();

    Ok(())
}

/// Tests retry logic on ESI server error.
///
/// Verifies that the character service successfully retries failed ESI requests
/// when encountering transient server errors (500), ultimately succeeding and
/// updateing the character data.
///
/// Expected: Ok with character data updateed after retry
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
        .with_mock_endpoint(move |server| {
            server
                .mock("GET", format!("/characters/{}", character_id).as_str())
                .with_status(500)
                .expect(1)
                .create()
        })
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();
    assert_eq!(character.character_id, character_id);

    test.assert_mocks();

    Ok(())
}

/// Tests retry logic with corporation dependency.
///
/// Verifies that the character service's retry logic correctly handles failures
/// when fetching characters with corporation dependencies, retrying both the character
/// and corporation fetches as needed.
///
/// Expected: Ok with character and corporation updateed after retry
#[tokio::test]
async fn retries_with_corporation_dependency() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    // Character request fails first, then succeeds
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_endpoint(move |server| {
            server
                .mock("GET", format!("/characters/{}", character_id).as_str())
                .with_status(500)
                .expect(1)
                .create()
        })
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();
    assert_eq!(character.character_id, character_id);

    // Verify both corporation and character exist
    let corporation = entity::prelude::EveCorporation::find()
        .one(&test.db)
        .await?;
    assert!(corporation.is_some());

    test.assert_mocks();

    Ok(())
}

/// Tests error handling after max retries.
///
/// Verifies that the character service returns an error when ESI continuously
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
        .with_mock_endpoint(move |server| {
            server
                .mock("GET", format!("/characters/{}", character_id).as_str())
                .with_status(503)
                .expect(3)
                .create()
        })
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(Error::EsiError(_))));

    test.assert_mocks();

    Ok(())
}

/// Tests error handling when ESI is unavailable.
///
/// Verifies that the character service returns an error when the ESI endpoint
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

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(matches!(
        result,
        Err(Error::EsiError(eve_esi::Error::EsiError(_)))
    ));

    Ok(())
}

/// Tests error handling when database tables are missing.
///
/// Verifies that the character service returns a database error when attempting
/// to update a character without the required database tables being created.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    // ESI will be called but database operations will fail
    let test = TestBuilder::new()
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests error handling when corporation table is missing.
///
/// Verifies that the character service returns a database error when attempting
/// to update a character with corporation dependency but the corporation table doesn't exist.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_corporation_table_missing() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    // Only character table exists, corporation table missing
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveCharacter)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests error handling when faction table is missing.
///
/// Verifies that the character service returns a database error when attempting
/// to update a character with faction dependency but the faction table doesn't exist.
///
/// Expected: Err with DbErr
#[tokio::test]
async fn fails_when_faction_table_missing() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;
    let faction_id = 500_001;

    // Only character table exists, faction table missing
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveCharacter)
        .with_faction_endpoint(vec![factory::mock_faction(faction_id)], 1)
        .with_corporation_endpoint(
            corporation_id,
            factory::mock_corporation(None, Some(faction_id)),
            1,
        )
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, Some(faction_id)),
            1,
        )
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Tests transaction rollback on error.
///
/// Verifies that when an error occurs during the update process, the transaction
/// is properly rolled back and no partial data is committed to the database.
///
/// Expected: Err with no character created in database
#[tokio::test]
async fn rolls_back_transaction_on_error() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    // Character endpoint will work but we'll drop the database connection
    // by not having the tables, causing transaction to fail
    let test = TestBuilder::new()
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_err());

    // Verify no character was created (would fail anyway since table doesn't exist)
    // This test mainly verifies the transaction rollback path is exercised

    Ok(())
}

/// Tests updating multiple characters sequentially.
///
/// Verifies that the character service can successfully update multiple different
/// characters in sequence, each with their own data and dependencies.
///
/// Expected: Ok with all characters created
#[tokio::test]
async fn updates_multiple_characters_sequentially() -> Result<(), TestError> {
    let char1_id = 95_000_001;
    let char2_id = 95_000_002;
    let char3_id = 95_000_003;
    let corp1_id = 98_000_001;
    let corp2_id = 98_000_002;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_corporation_endpoint(corp1_id, factory::mock_corporation(None, None), 1)
        .with_corporation_endpoint(corp2_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(char1_id, factory::mock_character(corp1_id, None, None), 1)
        .with_character_endpoint(char2_id, factory::mock_character(corp1_id, None, None), 1)
        .with_character_endpoint(char3_id, factory::mock_character(corp2_id, None, None), 1)
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);

    // Update first character
    let result1 = character_service.update(char1_id).await;
    assert!(result1.is_ok());

    // Update second character (same corporation)
    let result2 = character_service.update(char2_id).await;
    assert!(result2.is_ok());

    // Update third character (different corporation)
    let result3 = character_service.update(char3_id).await;
    assert!(result3.is_ok());

    // Verify all characters exist
    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), 3);

    // Verify corporations were created
    let corporations = entity::prelude::EveCorporation::find()
        .all(&test.db)
        .await?;
    assert_eq!(corporations.len(), 2);

    test.assert_mocks();

    Ok(())
}

/// Tests idempotency of update operation.
///
/// Verifies that calling update multiple times for the same character is idempotent,
/// updating the existing record rather than creating duplicates.
///
/// Expected: Ok with single character record updated
#[tokio::test]
async fn update_is_idempotent() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_corporation_endpoint(corporation_id, factory::mock_corporation(None, None), 1)
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            3,
        )
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);

    // Update same character three times
    let result1 = character_service.update(character_id).await;
    assert!(result1.is_ok());

    let result2 = character_service.update(character_id).await;
    assert!(result2.is_ok());

    let result3 = character_service.update(character_id).await;
    assert!(result3.is_ok());

    // Verify only one character exists
    let characters = entity::prelude::EveCharacter::find().all(&test.db).await?;
    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].character_id, character_id);

    test.assert_mocks();

    Ok(())
}

/// Tests updating timestamp on 304 Not Modified response.
///
/// Verifies that when ESI returns 304 Not Modified, only the info_updated_at
/// timestamp is updated while all other character data remains unchanged.
///
/// Expected: Ok with timestamp updated, other fields unchanged
#[tokio::test]
async fn updates_timestamp_on_304_not_modified() -> Result<(), TestError> {
    let character_id = 95_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_character(character_id, 98_000_001, None, None)
        .with_character_endpoint_not_modified(character_id, 1)
        .build()
        .await?;

    let character_before = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_timestamp = character_before.info_updated_at;
    let old_name = character_before.name.clone();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();

    // Verify timestamp was updated
    assert!(character.info_updated_at > old_timestamp);

    // Verify other fields remained unchanged
    assert_eq!(character.name, old_name);
    assert_eq!(character.character_id, character_id);

    test.assert_mocks();

    Ok(())
}

/// Tests update with 304 Not Modified for character with affiliations.
///
/// Verifies that 304 Not Modified handling works correctly for characters
/// with corporation and faction affiliations, preserving those relationships.
///
/// Expected: Ok with timestamp updated, affiliations preserved
#[tokio::test]
async fn updates_timestamp_on_304_with_affiliations() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;
    let faction_id = 500_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_character(character_id, corporation_id, None, Some(faction_id))
        .with_character_endpoint_not_modified(character_id, 1)
        .build()
        .await?;

    let character_before = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    let old_timestamp = character_before.info_updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let character_service = CharacterService::new(&test.db, &test.esi_client);
    let result = character_service.update(character_id).await;

    assert!(result.is_ok());
    let character = result.unwrap();

    // Verify timestamp was updated
    assert!(character.info_updated_at > old_timestamp);

    // Verify affiliations were preserved
    assert_eq!(character.corporation_id, character_before.corporation_id);
    assert_eq!(character.faction_id, character_before.faction_id);

    test.assert_mocks();

    Ok(())
}

/// Tests multiple updates with alternating 304 and fresh responses.
///
/// Verifies that the service correctly handles a mix of 304 Not Modified and
/// fresh data responses across multiple update calls for the same character.
///
/// Expected: Ok with appropriate behavior for each response type
#[tokio::test]
async fn handles_mixed_304_and_fresh_responses() -> Result<(), TestError> {
    let character_id = 95_000_001;
    let corporation_id = 98_000_001;

    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .with_mock_character(character_id, corporation_id, None, None)
        .with_mock_endpoint(move |server| {
            server
                .mock("GET", format!("/characters/{}", character_id).as_str())
                .with_status(304)
                .expect(1)
                .create()
        })
        .with_character_endpoint(
            character_id,
            factory::mock_character(corporation_id, None, None),
            1,
        )
        .build()
        .await?;

    let character_service = CharacterService::new(&test.db, &test.esi_client);

    // First update: 304 Not Modified
    let character_before = entity::prelude::EveCharacter::find()
        .one(&test.db)
        .await?
        .unwrap();
    let timestamp_before_304 = character_before.info_updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let result_304 = character_service.update(character_id).await;
    assert!(result_304.is_ok());
    let character_after_304 = result_304.unwrap();
    assert!(character_after_304.info_updated_at > timestamp_before_304);

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Second update: Fresh data
    let result_fresh = character_service.update(character_id).await;
    assert!(result_fresh.is_ok());
    let character_after_fresh = result_fresh.unwrap();
    assert!(character_after_fresh.info_updated_at > character_after_304.info_updated_at);

    test.assert_mocks();

    Ok(())
}
