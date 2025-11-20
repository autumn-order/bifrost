use super::*;

/// Expect Ok when updating affiliations for a single character with all entities existing
#[tokio::test]
async fn updates_affiliations_for_single_character() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;
    let _character = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    let mock_affiliation = test
        .eve()
        .with_mock_character_affiliation(2114794365, 98000001, None, None);
    let _affiliation_endpoint = test
        .eve()
        .with_character_affiliation_endpoint(vec![mock_affiliation], 1);

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let result = service.update_affiliations(vec![2114794365]).await;

    assert!(result.is_ok());

    // Verify character affiliation was updated
    let updated_character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_character.is_some());
    assert_eq!(updated_character.unwrap().corporation_id, corporation.id);

    Ok(())
}

/// Expect Ok when updating affiliations with alliance and faction
#[tokio::test]
async fn updates_affiliations_with_alliance_and_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let faction = test.eve().insert_mock_faction(500001).await?;
    let alliance = test
        .eve()
        .insert_mock_alliance(99000001, Some(500001))
        .await?;
    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, Some(99000001), Some(500001))
        .await?;
    let _character = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    let mock_affiliation = test.eve().with_mock_character_affiliation(
        2114794365,
        98000001,
        Some(99000001),
        Some(500001),
    );
    let _affiliation_endpoint = test
        .eve()
        .with_character_affiliation_endpoint(vec![mock_affiliation], 1);

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let result = service.update_affiliations(vec![2114794365]).await;

    assert!(result.is_ok());

    // Verify character affiliation was updated with faction
    let updated_character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_character.is_some());
    let updated_character = updated_character.unwrap();
    assert_eq!(updated_character.corporation_id, corporation.id);
    assert_eq!(updated_character.faction_id, Some(faction.id));

    // Verify corporation affiliation was updated with alliance
    let updated_corporation = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(updated_corporation.is_some());
    assert_eq!(updated_corporation.unwrap().alliance_id, Some(alliance.id));

    Ok(())
}

/// Expect Ok when fetching and storing missing entities
#[tokio::test]
async fn fetches_and_stores_missing_entities() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    // Set up mocks for entities that don't exist yet
    let (corporation_id, mock_corporation) = test.eve().with_mock_corporation(98000001, None, None);
    let _corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let (character_id, mock_character) = test
        .eve()
        .with_mock_character(2114794365, 98000001, None, None);
    let _character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let mock_affiliation = test
        .eve()
        .with_mock_character_affiliation(2114794365, 98000001, None, None);
    let _affiliation_endpoint = test
        .eve()
        .with_character_affiliation_endpoint(vec![mock_affiliation], 1);

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let result = service.update_affiliations(vec![2114794365]).await;

    assert!(result.is_ok());

    // Verify corporation was created
    let corporation = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(corporation.is_some());

    // Verify character was created
    let character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(character.is_some());

    Ok(())
}

/// Expect Ok when updating multiple characters
#[tokio::test]
async fn updates_affiliations_for_multiple_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let corporation1 = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;
    let corporation2 = test
        .eve()
        .insert_mock_corporation(98000002, None, None)
        .await?;
    let _character1 = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;
    let _character2 = test
        .eve()
        .insert_mock_character(2114794366, 98000002, None, None)
        .await?;

    let mock_affiliations = vec![
        test.eve()
            .with_mock_character_affiliation(2114794365, 98000001, None, None),
        test.eve()
            .with_mock_character_affiliation(2114794366, 98000002, None, None),
    ];
    let _affiliation_endpoint = test
        .eve()
        .with_character_affiliation_endpoint(mock_affiliations, 1);

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let result = service
        .update_affiliations(vec![2114794365, 2114794366])
        .await;

    assert!(result.is_ok());

    // Verify both characters were updated
    let updated_char1 = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_char1.is_some());
    assert_eq!(updated_char1.unwrap().corporation_id, corporation1.id);

    let updated_char2 = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794366)
        .await?;
    assert!(updated_char2.is_some());
    assert_eq!(updated_char2.unwrap().corporation_id, corporation2.id);

    Ok(())
}

/// Expect Ok when handling empty input
#[tokio::test]
async fn handles_empty_character_list() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let _affiliation_endpoint = test.eve().with_character_affiliation_endpoint(vec![], 1);

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let result = service.update_affiliations(vec![]).await;

    assert!(result.is_ok());

    Ok(())
}

/// Expect Ok when filtering out invalid character IDs
#[tokio::test]
async fn filters_invalid_character_ids() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;
    let _character = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    // Only valid character ID should be processed
    let mock_affiliation = test
        .eve()
        .with_mock_character_affiliation(2114794365, 98000001, None, None);
    let _affiliation_endpoint = test
        .eve()
        .with_character_affiliation_endpoint(vec![mock_affiliation], 1);

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    // Mix valid and invalid character IDs
    let result = service
        .update_affiliations(vec![
            123,         // Invalid: too low
            2114794365,  // Valid
            99999999999, // Invalid: too high
        ])
        .await;

    assert!(result.is_ok());

    // Verify valid character was updated
    let updated_character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_character.is_some());
    assert_eq!(updated_character.unwrap().corporation_id, corporation.id);

    Ok(())
}

/// Expect Ok when updating corporations and characters together
#[tokio::test]
async fn updates_both_corporation_and_character_affiliations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;
    let _character = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    // Corporation joins alliance
    let mock_affiliation =
        test.eve()
            .with_mock_character_affiliation(2114794365, 98000001, Some(99000001), None);
    let _affiliation_endpoint = test
        .eve()
        .with_character_affiliation_endpoint(vec![mock_affiliation], 1);

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let result = service.update_affiliations(vec![2114794365]).await;

    assert!(result.is_ok());

    // Verify character affiliation was updated
    let updated_character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_character.is_some());
    assert_eq!(updated_character.unwrap().corporation_id, corporation.id);

    // Verify corporation affiliation was updated with alliance
    let updated_corporation = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(updated_corporation.is_some());
    assert_eq!(updated_corporation.unwrap().alliance_id, Some(alliance.id));

    Ok(())
}

/// Expect Ok when characters from same corporation deduplicate corporation updates
#[tokio::test]
async fn deduplicates_corporation_updates() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;
    let _character1 = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;
    let _character2 = test
        .eve()
        .insert_mock_character(2114794366, 98000001, None, None)
        .await?;

    // Multiple characters in same corporation
    let mock_affiliations = vec![
        test.eve()
            .with_mock_character_affiliation(2114794365, 98000001, Some(99000001), None),
        test.eve()
            .with_mock_character_affiliation(2114794366, 98000001, Some(99000001), None),
    ];
    let _affiliation_endpoint = test
        .eve()
        .with_character_affiliation_endpoint(mock_affiliations, 1);

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let result = service
        .update_affiliations(vec![2114794365, 2114794366])
        .await;

    assert!(result.is_ok());

    // Verify both characters were updated
    let updated_char1 = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_char1.is_some());
    assert_eq!(updated_char1.unwrap().corporation_id, corporation.id);

    let updated_char2 = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794366)
        .await?;
    assert!(updated_char2.is_some());
    assert_eq!(updated_char2.unwrap().corporation_id, corporation.id);

    // Verify corporation was updated with alliance (should only happen once despite two characters)
    let updated_corporation = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(updated_corporation.is_some());
    assert_eq!(updated_corporation.unwrap().alliance_id, Some(alliance.id));

    Ok(())
}

/// Expect Ok when handling complex entity relationships
#[tokio::test]
async fn handles_complex_entity_relationships() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    // Set up faction
    let mock_factions = vec![test.eve().with_mock_faction(500001)];
    let _faction_endpoint = test.eve().with_faction_endpoint(mock_factions, 1);

    // Set up alliance with faction
    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(99000001, Some(500001));
    let _alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    // Set up corporation with alliance and faction
    let (corporation_id, mock_corporation) =
        test.eve()
            .with_mock_corporation(98000001, Some(99000001), Some(500001));
    let _corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    // Set up character
    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(2114794365, 98000001, Some(99000001), Some(500001));
    let _character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let mock_affiliation = test.eve().with_mock_character_affiliation(
        2114794365,
        98000001,
        Some(99000001),
        Some(500001),
    );
    let _affiliation_endpoint = test
        .eve()
        .with_character_affiliation_endpoint(vec![mock_affiliation], 1);

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let result = service.update_affiliations(vec![2114794365]).await;

    assert!(result.is_ok());

    // Verify all entities were created with proper relationships
    let faction = FactionRepository::new(test.state.db.clone())
        .get_by_faction_id(500001)
        .await?;
    assert!(faction.is_some());

    let alliance = AllianceRepository::new(test.state.db.clone())
        .get_by_alliance_id(99000001)
        .await?;
    assert!(alliance.is_some());
    assert_eq!(
        alliance.as_ref().unwrap().faction_id,
        Some(faction.unwrap().id)
    );

    let corporation = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(corporation.is_some());
    assert_eq!(
        corporation.as_ref().unwrap().alliance_id,
        Some(alliance.unwrap().id)
    );

    let character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(character.is_some());
    assert_eq!(
        character.as_ref().unwrap().corporation_id,
        corporation.unwrap().id
    );

    Ok(())
}
