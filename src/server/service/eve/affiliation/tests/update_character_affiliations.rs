use super::*;

/// Expect Ok when updating character affiliations with valid references
#[tokio::test]
async fn updates_character_affiliations_successfully() -> Result<(), TestError> {
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
    let character = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: vec![(2114794365, character.id)].into_iter().collect(),
    };

    let affiliations = vec![CharacterAffiliation {
        character_id: 2114794365,
        corporation_id: 98000001,
        alliance_id: None,
        faction_id: None,
    }];

    let result = service
        .update_character_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify the database was updated
    let updated_character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_character.is_some());
    let updated_character = updated_character.unwrap();
    assert_eq!(updated_character.corporation_id, corporation.id);
    assert_eq!(updated_character.faction_id, None);

    Ok(())
}

/// Expect Ok when updating character affiliations with faction references
#[tokio::test]
async fn updates_character_affiliations_with_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let faction = test.eve().insert_mock_faction(500001).await?;
    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;
    let character = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: vec![(500001, faction.id)].into_iter().collect(),
        alliance_ids: HashMap::new(),
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: vec![(2114794365, character.id)].into_iter().collect(),
    };

    let affiliations = vec![CharacterAffiliation {
        character_id: 2114794365,
        corporation_id: 98000001,
        alliance_id: None,
        faction_id: Some(500001),
    }];

    let result = service
        .update_character_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify the database was updated with faction
    let updated_character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_character.is_some());
    let updated_character = updated_character.unwrap();
    assert_eq!(updated_character.corporation_id, corporation.id);
    assert_eq!(updated_character.faction_id, Some(faction.id));

    Ok(())
}

/// Expect Ok but skip affiliations when character is not found
#[tokio::test]
async fn skips_affiliations_when_character_missing() -> Result<(), TestError> {
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

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: HashMap::new(), // Character not in table_ids
    };

    let affiliations = vec![CharacterAffiliation {
        character_id: 2114794365,
        corporation_id: 98000001,
        alliance_id: None,
        faction_id: None,
    }];

    let result = service
        .update_character_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify no character was created/updated
    let character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(character.is_none());

    Ok(())
}

/// Expect Ok but skip affiliations when corporation is not found
#[tokio::test]
async fn skips_affiliations_when_corporation_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let character = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    let original_corporation_id = character.corporation_id;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: HashMap::new(), // Corporation not in table_ids
        character_ids: vec![(2114794365, character.id)].into_iter().collect(),
    };

    let affiliations = vec![CharacterAffiliation {
        character_id: 2114794365,
        corporation_id: 98000001,
        alliance_id: None,
        faction_id: None,
    }];

    let result = service
        .update_character_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify character was not updated (corporation_id should remain unchanged)
    let character_after = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(character_after.is_some());
    assert_eq!(
        character_after.unwrap().corporation_id,
        original_corporation_id
    );

    Ok(())
}

/// Expect Ok and set faction to None when faction is not found
#[tokio::test]
async fn sets_faction_to_none_when_faction_missing() -> Result<(), TestError> {
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
    let character = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(), // Faction not in table_ids
        alliance_ids: HashMap::new(),
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: vec![(2114794365, character.id)].into_iter().collect(),
    };

    let affiliations = vec![CharacterAffiliation {
        character_id: 2114794365,
        corporation_id: 98000001,
        alliance_id: None,
        faction_id: Some(500001), // Faction not found, should be set to None
    }];

    let result = service
        .update_character_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify faction was set to None
    let updated_character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_character.is_some());
    let updated_character = updated_character.unwrap();
    assert_eq!(updated_character.faction_id, None);

    Ok(())
}

/// Expect Ok when updating multiple character affiliations
#[tokio::test]
async fn updates_multiple_character_affiliations() -> Result<(), TestError> {
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
    let character1 = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;
    let character2 = test
        .eve()
        .insert_mock_character(2114794366, 98000002, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: vec![(98000001, corporation1.id), (98000002, corporation2.id)]
            .into_iter()
            .collect(),
        character_ids: vec![(2114794365, character1.id), (2114794366, character2.id)]
            .into_iter()
            .collect(),
    };

    let affiliations = vec![
        CharacterAffiliation {
            character_id: 2114794365,
            corporation_id: 98000001,
            alliance_id: None,
            faction_id: None,
        },
        CharacterAffiliation {
            character_id: 2114794366,
            corporation_id: 98000002,
            alliance_id: None,
            faction_id: None,
        },
    ];

    let result = service
        .update_character_affiliations(&affiliations, &table_ids)
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

/// Expect Ok when deduplicating character affiliations
#[tokio::test]
async fn deduplicates_character_affiliations() -> Result<(), TestError> {
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
    let character = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: vec![(2114794365, character.id)].into_iter().collect(),
    };

    // Duplicate affiliations
    let affiliations = vec![
        CharacterAffiliation {
            character_id: 2114794365,
            corporation_id: 98000001,
            alliance_id: None,
            faction_id: None,
        },
        CharacterAffiliation {
            character_id: 2114794365,
            corporation_id: 98000001,
            alliance_id: None,
            faction_id: None,
        },
    ];

    let result = service
        .update_character_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify character was updated (deduplication should handle duplicates)
    let updated_character = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_character.is_some());
    let updated_character = updated_character.unwrap();
    assert_eq!(updated_character.corporation_id, corporation.id);

    Ok(())
}

/// Expect Ok when processing empty affiliations list
#[tokio::test]
async fn handles_empty_affiliations_list() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: HashMap::new(),
        character_ids: HashMap::new(),
    };

    let affiliations: Vec<CharacterAffiliation> = vec![];

    let result = service
        .update_character_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify no updates occurred (should be no-op for empty list)
    // This test just ensures the method handles empty input gracefully
    // No database state to verify since no entities were involved

    Ok(())
}

/// Expect Ok when processing mixed valid and invalid affiliations
#[tokio::test]
async fn processes_mixed_valid_and_invalid_affiliations() -> Result<(), TestError> {
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
    let character1 = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: vec![(2114794365, character1.id)].into_iter().collect(),
    };

    let affiliations = vec![
        // Valid affiliation
        CharacterAffiliation {
            character_id: 2114794365,
            corporation_id: 98000001,
            alliance_id: None,
            faction_id: None,
        },
        // Invalid - character not found
        CharacterAffiliation {
            character_id: 9999999999,
            corporation_id: 98000001,
            alliance_id: None,
            faction_id: None,
        },
        // Invalid - corporation not found
        CharacterAffiliation {
            character_id: 2114794365,
            corporation_id: 9999999999,
            alliance_id: None,
            faction_id: None,
        },
    ];

    let result = service
        .update_character_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify valid affiliation was processed
    let updated_char = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(2114794365)
        .await?;
    assert!(updated_char.is_some());
    assert_eq!(updated_char.unwrap().corporation_id, corporation.id);

    // Verify invalid character was not created
    let invalid_char = CharacterRepository::new(test.state.db.clone())
        .get_by_character_id(9999999999)
        .await?;
    assert!(invalid_char.is_none());

    Ok(())
}
