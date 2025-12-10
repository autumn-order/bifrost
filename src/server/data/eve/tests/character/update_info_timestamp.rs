//! Tests for CharacterRepository::update_info_timestamp method.
//!
//! This module verifies the character info timestamp update behavior,
//! including updating existing characters and handling non-existent characters.

use super::*;

/// Tests updating info timestamp for an existing character.
///
/// Verifies that the character repository successfully updates the info_updated_at
/// timestamp to the current time for an existing character record.
///
/// Expected: Ok with updated character having newer info_updated_at timestamp
#[tokio::test]
async fn updates_info_timestamp_for_existing_character() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;
    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id, character) =
        test.eve()
            .mock_character(1, corporation.corporation_id, None, None);

    let character_repo = CharacterRepository::new(&test.db);
    let created = character_repo
        .upsert_many(vec![(character_id, character, corporation.id, None)])
        .await?;

    let created_char = &created[0];
    let initial_timestamp = created_char.info_updated_at;

    // Wait a moment to ensure timestamp changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let result = character_repo.update_info_timestamp(created_char.id).await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let updated_char = result.unwrap();

    assert_eq!(updated_char.id, created_char.id);
    assert_eq!(updated_char.character_id, character_id);
    assert!(
        updated_char.info_updated_at > initial_timestamp,
        "info_updated_at should be newer: {:?} vs {:?}",
        updated_char.info_updated_at,
        initial_timestamp
    );

    Ok(())
}

/// Tests updating info timestamp returns error for non-existent character.
///
/// Verifies that the character repository returns an error when attempting
/// to update the info timestamp for a character ID that doesn't exist.
///
/// Expected: Err(DbErr::RecordNotFound)
#[tokio::test]
async fn returns_error_for_nonexistent_character() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    let character_repo = CharacterRepository::new(&test.db);
    let result = character_repo.update_info_timestamp(999999).await;

    assert!(result.is_err());
    match result {
        Err(sea_orm::DbErr::RecordNotFound(_)) => (),
        _ => panic!("Expected DbErr::RecordNotFound, got: {:?}", result),
    }

    Ok(())
}

/// Tests that update_info_timestamp only updates the timestamp field.
///
/// Verifies that updating the info timestamp doesn't modify any other
/// character data fields like name, corporation_id, security_status, etc.
///
/// Expected: Ok with all fields unchanged except info_updated_at
#[tokio::test]
async fn only_updates_timestamp_field() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;
    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id, character) =
        test.eve()
            .mock_character(1, corporation.corporation_id, None, None);

    let character_repo = CharacterRepository::new(&test.db);
    let created = character_repo
        .upsert_many(vec![(
            character_id,
            character.clone(),
            corporation.id,
            None,
        )])
        .await?;

    let created_char = &created[0];

    // Wait a moment to ensure timestamp changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let updated_char = character_repo
        .update_info_timestamp(created_char.id)
        .await?;

    // Verify all fields except info_updated_at remain the same
    assert_eq!(updated_char.id, created_char.id);
    assert_eq!(updated_char.character_id, created_char.character_id);
    assert_eq!(updated_char.name, created_char.name);
    assert_eq!(updated_char.corporation_id, created_char.corporation_id);
    assert_eq!(updated_char.faction_id, created_char.faction_id);
    assert_eq!(updated_char.birthday, created_char.birthday);
    assert_eq!(updated_char.bloodline_id, created_char.bloodline_id);
    assert_eq!(updated_char.description, created_char.description);
    assert_eq!(updated_char.gender, created_char.gender);
    assert_eq!(updated_char.race_id, created_char.race_id);
    assert_eq!(updated_char.security_status, created_char.security_status);
    assert_eq!(updated_char.title, created_char.title);
    assert_eq!(updated_char.created_at, created_char.created_at);
    assert_eq!(
        updated_char.affiliation_updated_at,
        created_char.affiliation_updated_at
    );

    // Only info_updated_at should change
    assert!(updated_char.info_updated_at > created_char.info_updated_at);

    Ok(())
}

/// Tests updating info timestamp multiple times.
///
/// Verifies that the info timestamp can be updated multiple times and
/// each update results in a newer timestamp.
///
/// Expected: Ok with progressively newer timestamps on each update
#[tokio::test]
async fn handles_multiple_updates() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;
    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id, character) =
        test.eve()
            .mock_character(1, corporation.corporation_id, None, None);

    let character_repo = CharacterRepository::new(&test.db);
    let created = character_repo
        .upsert_many(vec![(character_id, character, corporation.id, None)])
        .await?;

    let created_char = &created[0];
    let mut previous_timestamp = created_char.info_updated_at;

    // Update timestamp 3 times
    for _ in 0..3 {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let updated = character_repo
            .update_info_timestamp(created_char.id)
            .await?;

        assert!(
            updated.info_updated_at > previous_timestamp,
            "Each update should result in a newer timestamp"
        );
        previous_timestamp = updated.info_updated_at;
    }

    Ok(())
}

/// Tests updating info timestamp for characters with affiliations.
///
/// Verifies that updating the info timestamp works correctly for characters
/// that have corporation and faction affiliations.
///
/// Expected: Ok with updated timestamp and preserved affiliations
#[tokio::test]
async fn updates_timestamp_for_character_with_affiliations() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;
    let faction_id = 500_001;
    let faction = test.eve().insert_mock_faction(faction_id).await?;
    let corporation = test.eve().insert_mock_corporation(1, None, None).await?;
    let (character_id, character) =
        test.eve()
            .mock_character(1, corporation.corporation_id, None, Some(faction_id));

    let character_repo = CharacterRepository::new(&test.db);
    let created = character_repo
        .upsert_many(vec![(
            character_id,
            character,
            corporation.id,
            Some(faction.id),
        )])
        .await?;

    let created_char = &created[0];
    let initial_timestamp = created_char.info_updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let updated = character_repo
        .update_info_timestamp(created_char.id)
        .await?;

    assert!(updated.info_updated_at > initial_timestamp);
    assert_eq!(updated.corporation_id, corporation.id);
    assert_eq!(updated.faction_id, Some(faction.id));

    Ok(())
}
