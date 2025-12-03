//! Tests for CharacterRepository::update_affiliations method.
//!
//! This module verifies the character affiliation update behavior, including updating
//! corporation and faction affiliations for single and multiple characters, handling
//! batch operations, timestamp updates, and edge cases like empty inputs and mixed faction assignments.

use super::*;
use sea_orm::EntityTrait;

/// Tests updating a single character's affiliation.
///
/// Verifies that the character repository successfully updates a character's
/// corporation and faction affiliations in the database.
///
/// Expected: Ok with updated corporation_id and faction_id
#[tokio::test]
async fn updates_single_character_affiliation() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    // Create factions and corporations
    let faction1 = test.eve().insert_mock_faction(1).await?;
    let faction2 = test.eve().insert_mock_faction(2).await?;
    let corp1 = test
        .eve()
        .insert_mock_corporation(100, None, Some(faction1.faction_id))
        .await?;
    let corp2 = test
        .eve()
        .insert_mock_corporation(200, None, Some(faction2.faction_id))
        .await?;

    // Create a character initially affiliated with corp1 and faction1
    let (character_id, character) = test
        .eve()
        .mock_character(1, corp1.corporation_id, None, None);
    let character_repo = CharacterRepository::new(&test.db);
    let chars = character_repo
        .upsert_many(vec![(character_id, character, corp1.id, Some(faction1.id))])
        .await?;
    let char = chars
        .into_iter()
        .next()
        .expect("Character should be created");

    // Update character to be affiliated with corp2 and faction2
    let result = character_repo
        .update_affiliations(vec![(char.id, corp2.id, Some(faction2.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the update by querying directly
    let updated = entity::prelude::EveCharacter::find_by_id(char.id)
        .one(&test.db)
        .await?
        .expect("Character should exist");

    assert_eq!(updated.corporation_id, corp2.id);
    assert_eq!(updated.faction_id, Some(faction2.id));

    Ok(())
}

/// Tests updating multiple characters in a single call.
///
/// Verifies that the character repository successfully updates affiliations for
/// multiple characters in a single batch operation.
///
/// Expected: Ok with all characters updated to their respective affiliations
#[tokio::test]
async fn updates_multiple_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    // Create factions and corporations
    let faction1 = test.eve().insert_mock_faction(1).await?;
    let faction2 = test.eve().insert_mock_faction(2).await?;
    let faction3 = test.eve().insert_mock_faction(3).await?;
    let corp1 = test.eve().insert_mock_corporation(100, None, None).await?;
    let corp2 = test.eve().insert_mock_corporation(200, None, None).await?;
    let corp3 = test.eve().insert_mock_corporation(300, None, None).await?;

    // Create characters
    let char1 = test
        .eve()
        .insert_mock_character(1, corp1.corporation_id, None, None)
        .await?;

    let char2 = test
        .eve()
        .insert_mock_character(2, corp1.corporation_id, None, None)
        .await?;

    let char3 = test
        .eve()
        .insert_mock_character(3, corp1.corporation_id, None, None)
        .await?;

    // Update multiple characters
    let character_repo = CharacterRepository::new(&test.db);
    let result = character_repo
        .update_affiliations(vec![
            (char1.id, corp1.id, Some(faction1.id)),
            (char2.id, corp2.id, Some(faction2.id)),
            (char3.id, corp3.id, Some(faction3.id)),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify all updates by querying directly
    let updated1 = entity::prelude::EveCharacter::find_by_id(char1.id)
        .one(&test.db)
        .await?
        .expect("Character 1 should exist");
    let updated2 = entity::prelude::EveCharacter::find_by_id(char2.id)
        .one(&test.db)
        .await?
        .expect("Character 2 should exist");
    let updated3 = entity::prelude::EveCharacter::find_by_id(char3.id)
        .one(&test.db)
        .await?
        .expect("Character 3 should exist");

    assert_eq!(updated1.corporation_id, corp1.id);
    assert_eq!(updated1.faction_id, Some(faction1.id));
    assert_eq!(updated2.corporation_id, corp2.id);
    assert_eq!(updated2.faction_id, Some(faction2.id));
    assert_eq!(updated3.corporation_id, corp3.id);
    assert_eq!(updated3.faction_id, Some(faction3.id));

    Ok(())
}

/// Tests removing faction affiliation.
///
/// Verifies that the character repository successfully removes a character's
/// faction affiliation by setting it to None while maintaining corporation affiliation.
///
/// Expected: Ok with faction_id set to None
#[tokio::test]
async fn removes_faction_affiliation() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    // Create faction and corporation
    let faction = test.eve().insert_mock_faction(1).await?;
    let corp = test.eve().insert_mock_corporation(100, None, None).await?;

    // Create a character with a faction
    let (character_id, character) = test
        .eve()
        .mock_character(1, corp.corporation_id, None, None);
    let character_repo = CharacterRepository::new(&test.db);
    let chars = character_repo
        .upsert_many(vec![(character_id, character, corp.id, Some(faction.id))])
        .await?;
    let char = chars
        .into_iter()
        .next()
        .expect("Character should be created");

    // Remove faction affiliation
    let result = character_repo
        .update_affiliations(vec![(char.id, corp.id, None)])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the faction was removed
    let updated = entity::prelude::EveCharacter::find_by_id(char.id)
        .one(&test.db)
        .await?
        .expect("Character should exist");

    assert_eq!(updated.faction_id, None);

    Ok(())
}

/// Tests handling large batch updates.
///
/// Verifies that the character repository correctly handles batching when updating
/// affiliations for large numbers of characters (>100), ensuring all updates are
/// processed across multiple batches.
///
/// Expected: Ok with all 250 characters updated
#[tokio::test]
async fn handles_large_batch_updates() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    // Create a corporation and faction
    let corp = test.eve().insert_mock_corporation(100, None, None).await?;
    let faction = test.eve().insert_mock_faction(1).await?;

    // Create 250 characters (more than 2x BATCH_SIZE)
    let mut characters = Vec::new();
    for i in 0..250 {
        let char = test
            .eve()
            .insert_mock_character(1000 + i, corp.corporation_id, None, None)
            .await?;

        characters.push((char.id, corp.id, Some(faction.id)));
    }

    // Update all characters
    let character_repo = CharacterRepository::new(&test.db);
    let result = character_repo.update_affiliations(characters).await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify a sample of updates using direct entity queries
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let updated_first = entity::prelude::EveCharacter::find()
        .filter(entity::eve_character::Column::CharacterId.eq(1000))
        .one(&test.db)
        .await?
        .expect("First character should exist");
    let updated_middle = entity::prelude::EveCharacter::find()
        .filter(entity::eve_character::Column::CharacterId.eq(1125))
        .one(&test.db)
        .await?
        .expect("Middle character should exist");
    let updated_last = entity::prelude::EveCharacter::find()
        .filter(entity::eve_character::Column::CharacterId.eq(1249))
        .one(&test.db)
        .await?
        .expect("Last character should exist");

    assert_eq!(updated_first.faction_id, Some(faction.id));
    assert_eq!(updated_middle.faction_id, Some(faction.id));
    assert_eq!(updated_last.faction_id, Some(faction.id));

    Ok(())
}

/// Tests handling empty input.
///
/// Verifies that the character repository handles empty affiliation update lists
/// gracefully without errors.
///
/// Expected: Ok with no operations performed
#[tokio::test]
async fn handles_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    let character_repo = CharacterRepository::new(&test.db);
    let result = character_repo.update_affiliations(vec![]).await;

    assert!(result.is_ok(), "Should handle empty input gracefully");

    Ok(())
}

/// Tests updating affiliation timestamp.
///
/// Verifies that the character repository updates the affiliation_updated_at
/// timestamp whenever affiliation data is modified.
///
/// Expected: Ok with affiliation_updated_at newer than original
#[tokio::test]
async fn updates_timestamp() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    // Create corporation and faction
    let corp = test.eve().insert_mock_corporation(100, None, None).await?;
    let faction = test.eve().insert_mock_faction(1).await?;

    // Create a character
    let (character_id, character) = test
        .eve()
        .mock_character(1, corp.corporation_id, None, None);
    let character_repo = CharacterRepository::new(&test.db);
    let chars = character_repo
        .upsert_many(vec![(character_id, character, corp.id, None)])
        .await?;
    let char = chars
        .into_iter()
        .next()
        .expect("Character should be created");

    let original_updated_at = char.affiliation_updated_at;

    // Wait a moment to ensure timestamp difference (Utc::now() has nanosecond precision)
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Update the character
    let result = character_repo
        .update_affiliations(vec![(char.id, corp.id, Some(faction.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the timestamp was updated
    let updated = entity::prelude::EveCharacter::find_by_id(char.id)
        .one(&test.db)
        .await?
        .expect("Character should exist");

    assert!(
        updated.affiliation_updated_at > original_updated_at,
        "affiliation_updated_at should be newer than original. Original: {:?}, Updated: {:?}",
        original_updated_at,
        updated.affiliation_updated_at
    );

    Ok(())
}

/// Tests that other characters are not affected.
///
/// Verifies that the character repository only updates characters specified in
/// the update list, leaving other characters' affiliations unchanged.
///
/// Expected: Ok with only specified character updated
#[tokio::test]
async fn does_not_affect_other_characters() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    // Create factions and corporations
    let faction1 = test.eve().insert_mock_faction(1).await?;
    let faction2 = test.eve().insert_mock_faction(2).await?;
    let corp1 = test.eve().insert_mock_corporation(100, None, None).await?;
    let corp2 = test.eve().insert_mock_corporation(200, None, None).await?;

    // Create characters
    let char1 = test
        .eve()
        .insert_mock_character(1, corp1.corporation_id, None, Some(faction1.faction_id))
        .await?;
    let char2 = test
        .eve()
        .insert_mock_character(2, corp1.corporation_id, None, Some(faction1.faction_id))
        .await?;

    // Update only char1
    let character_repo = CharacterRepository::new(&test.db);
    let result = character_repo
        .update_affiliations(vec![(char1.id, corp2.id, Some(faction2.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify char1 was updated
    let updated1 = entity::prelude::EveCharacter::find_by_id(char1.id)
        .one(&test.db)
        .await?
        .expect("Character 1 should exist");
    assert_eq!(updated1.corporation_id, corp2.id);
    assert_eq!(updated1.faction_id, Some(faction2.id));

    // Verify char2 was NOT updated
    let updated2 = entity::prelude::EveCharacter::find_by_id(char2.id)
        .one(&test.db)
        .await?
        .expect("Character 2 should exist");
    assert_eq!(
        updated2.corporation_id, corp1.id,
        "Character 2 should still have original corporation"
    );
    assert_eq!(
        updated2.faction_id,
        Some(faction1.id),
        "Character 2 should still have original faction"
    );

    Ok(())
}

/// Tests handling mixed faction assignments.
///
/// Verifies that the character repository correctly processes a batch containing
/// both Some and None faction IDs, applying each appropriately.
///
/// Expected: Ok with characters having correct faction assignments
#[tokio::test]
async fn handles_mixed_faction_assignments() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .with_table(entity::prelude::EveCharacter)
        .build()
        .await?;

    // Create faction and corporation
    let faction = test.eve().insert_mock_faction(1).await?;
    let corp = test.eve().insert_mock_corporation(100, None, None).await?;

    // Create characters
    let char1 = test
        .eve()
        .insert_mock_character(1, corp.corporation_id, None, None)
        .await?;
    let char2 = test
        .eve()
        .insert_mock_character(2, corp.corporation_id, None, None)
        .await?;
    let char3 = test
        .eve()
        .insert_mock_character(3, corp.corporation_id, None, None)
        .await?;

    // Update with mixed faction IDs
    let character_repo = CharacterRepository::new(&test.db);
    let result = character_repo
        .update_affiliations(vec![
            (char1.id, corp.id, Some(faction.id)),
            (char2.id, corp.id, None),
            (char3.id, corp.id, Some(faction.id)),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify updates
    let updated1 = entity::prelude::EveCharacter::find_by_id(char1.id)
        .one(&test.db)
        .await?
        .expect("Character 1 should exist");
    let updated2 = entity::prelude::EveCharacter::find_by_id(char2.id)
        .one(&test.db)
        .await?
        .expect("Character 2 should exist");
    let updated3 = entity::prelude::EveCharacter::find_by_id(char3.id)
        .one(&test.db)
        .await?
        .expect("Character 3 should exist");

    assert_eq!(updated1.faction_id, Some(faction.id));
    assert_eq!(updated2.faction_id, None);
    assert_eq!(updated3.faction_id, Some(faction.id));

    Ok(())
}
