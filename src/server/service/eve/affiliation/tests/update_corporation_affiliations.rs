use super::*;

/// Expect Ok when updating corporation affiliations with alliance
#[tokio::test]
async fn updates_corporation_affiliations_with_alliance() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
    )?;

    let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: HashMap::new(),
    };

    let affiliations = vec![CharacterAffiliation {
        character_id: 2114794365, // Character ID doesn't matter for corporation updates
        corporation_id: 98000001,
        alliance_id: Some(99000001),
        faction_id: None,
    }];

    let result = service
        .update_corporation_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify the database was updated
    let updated_corporation = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(updated_corporation.is_some());
    let updated_corporation = updated_corporation.unwrap();
    assert_eq!(updated_corporation.alliance_id, Some(alliance.id));

    Ok(())
}

/// Expect Ok when updating corporation affiliations without alliance
#[tokio::test]
async fn updates_corporation_affiliations_without_alliance() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
    )?;

    let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, Some(99000001), None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: HashMap::new(),
    };

    let affiliations = vec![CharacterAffiliation {
        character_id: 2114794365,
        corporation_id: 98000001,
        alliance_id: None, // Removing alliance affiliation
        faction_id: None,
    }];

    let result = service
        .update_corporation_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify the alliance was removed
    let updated_corporation = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(updated_corporation.is_some());
    let updated_corporation = updated_corporation.unwrap();
    assert_eq!(updated_corporation.alliance_id, None);

    Ok(())
}

/// Expect Ok but skip affiliations when corporation is not found
#[tokio::test]
async fn skips_affiliations_when_corporation_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
    )?;

    let alliance = test.eve().insert_mock_alliance(99000001, None).await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
        corporation_ids: HashMap::new(), // Corporation not in table_ids
        character_ids: HashMap::new(),
    };

    let affiliations = vec![CharacterAffiliation {
        character_id: 2114794365,
        corporation_id: 98000001,
        alliance_id: Some(99000001),
        faction_id: None,
    }];

    let result = service
        .update_corporation_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify no corporation was created/updated
    let corporation = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(corporation.is_none());

    Ok(())
}

/// Expect Ok but skip affiliations when alliance is not found
#[tokio::test]
async fn skips_affiliations_when_alliance_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
    )?;

    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;

    let original_alliance_id = corporation.alliance_id;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(), // Alliance not in table_ids
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: HashMap::new(),
    };

    let affiliations = vec![CharacterAffiliation {
        character_id: 2114794365,
        corporation_id: 98000001,
        alliance_id: Some(99000001), // Alliance not found
        faction_id: None,
    }];

    let result = service
        .update_corporation_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify corporation was not updated (alliance_id should remain unchanged)
    let corporation_after = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(corporation_after.is_some());
    assert_eq!(corporation_after.unwrap().alliance_id, original_alliance_id);

    Ok(())
}

/// Expect Ok when updating multiple corporation affiliations
#[tokio::test]
async fn updates_multiple_corporation_affiliations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
    )?;

    let alliance1 = test.eve().insert_mock_alliance(99000001, None).await?;
    let alliance2 = test.eve().insert_mock_alliance(99000002, None).await?;
    let corporation1 = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;
    let corporation2 = test
        .eve()
        .insert_mock_corporation(98000002, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: vec![(99000001, alliance1.id), (99000002, alliance2.id)]
            .into_iter()
            .collect(),
        corporation_ids: vec![(98000001, corporation1.id), (98000002, corporation2.id)]
            .into_iter()
            .collect(),
        character_ids: HashMap::new(),
    };

    let affiliations = vec![
        CharacterAffiliation {
            character_id: 2114794365,
            corporation_id: 98000001,
            alliance_id: Some(99000001),
            faction_id: None,
        },
        CharacterAffiliation {
            character_id: 2114794366,
            corporation_id: 98000002,
            alliance_id: Some(99000002),
            faction_id: None,
        },
    ];

    let result = service
        .update_corporation_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify both corporations were updated
    let updated_corp1 = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(updated_corp1.is_some());
    assert_eq!(updated_corp1.unwrap().alliance_id, Some(alliance1.id));

    let updated_corp2 = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000002)
        .await?;
    assert!(updated_corp2.is_some());
    assert_eq!(updated_corp2.unwrap().alliance_id, Some(alliance2.id));

    Ok(())
}

/// Expect Ok when deduplicating corporation affiliations
#[tokio::test]
async fn deduplicates_corporation_affiliations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
    )?;

    let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: HashMap::new(),
    };

    // Duplicate affiliations (from different characters in same corporation)
    let affiliations = vec![
        CharacterAffiliation {
            character_id: 2114794365,
            corporation_id: 98000001,
            alliance_id: Some(99000001),
            faction_id: None,
        },
        CharacterAffiliation {
            character_id: 2114794366, // Different character, same corporation
            corporation_id: 98000001,
            alliance_id: Some(99000001),
            faction_id: None,
        },
    ];

    let result = service
        .update_corporation_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify corporation was updated (deduplication should handle duplicates)
    let updated_corporation = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(updated_corporation.is_some());
    let updated_corporation = updated_corporation.unwrap();
    assert_eq!(updated_corporation.alliance_id, Some(alliance.id));

    Ok(())
}

/// Expect Ok when processing empty affiliations list
#[tokio::test]
async fn handles_empty_affiliations_list() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
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
        .update_corporation_affiliations(&affiliations, &table_ids)
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
    )?;

    let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
    let corporation1 = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
        corporation_ids: vec![(98000001, corporation1.id)].into_iter().collect(),
        character_ids: HashMap::new(),
    };

    let affiliations = vec![
        // Valid affiliation
        CharacterAffiliation {
            character_id: 2114794365,
            corporation_id: 98000001,
            alliance_id: Some(99000001),
            faction_id: None,
        },
        // Invalid - corporation not found
        CharacterAffiliation {
            character_id: 2114794366,
            corporation_id: 9999999999,
            alliance_id: Some(99000001),
            faction_id: None,
        },
        // Invalid - alliance not found
        CharacterAffiliation {
            character_id: 2114794367,
            corporation_id: 98000001,
            alliance_id: Some(9999999999),
            faction_id: None,
        },
    ];

    let result = service
        .update_corporation_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify valid affiliation was processed
    let updated_corp = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(updated_corp.is_some());
    assert_eq!(updated_corp.unwrap().alliance_id, Some(alliance.id));

    // Verify invalid corporation was not created
    let invalid_corp = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(9999999999)
        .await?;
    assert!(invalid_corp.is_none());

    Ok(())
}

/// Expect Ok when updating corporation with mixed alliance statuses
#[tokio::test]
async fn updates_corporation_with_mixed_alliance_statuses() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
    )?;

    let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
    let corporation1 = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;
    let corporation2 = test
        .eve()
        .insert_mock_corporation(98000002, Some(99000001), None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
        corporation_ids: vec![(98000001, corporation1.id), (98000002, corporation2.id)]
            .into_iter()
            .collect(),
        character_ids: HashMap::new(),
    };

    let affiliations = vec![
        // Add alliance to corporation1
        CharacterAffiliation {
            character_id: 2114794365,
            corporation_id: 98000001,
            alliance_id: Some(99000001),
            faction_id: None,
        },
        // Remove alliance from corporation2
        CharacterAffiliation {
            character_id: 2114794366,
            corporation_id: 98000002,
            alliance_id: None,
            faction_id: None,
        },
    ];

    let result = service
        .update_corporation_affiliations(&affiliations, &table_ids)
        .await;

    assert!(result.is_ok());

    // Verify corporation1 now has alliance
    let updated_corp1 = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000001)
        .await?;
    assert!(updated_corp1.is_some());
    assert_eq!(updated_corp1.unwrap().alliance_id, Some(alliance.id));

    // Verify corporation2 no longer has alliance
    let updated_corp2 = CorporationRepository::new(test.state.db.clone())
        .get_by_corporation_id(98000002)
        .await?;
    assert!(updated_corp2.is_some());
    assert_eq!(updated_corp2.unwrap().alliance_id, None);

    Ok(())
}
