use super::*;

/// Expect Ok with correct mappings when all entity types exist in database
#[tokio::test]
async fn returns_mappings_for_all_entity_types() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    // Insert test data
    let faction = test.eve().insert_mock_faction(500001).await?;
    let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;
    let character = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;

    // Create service and input
    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let unique_ids = UniqueIds {
        faction_ids: vec![faction.faction_id].into_iter().collect(),
        alliance_ids: vec![alliance.alliance_id].into_iter().collect(),
        corporation_ids: vec![corporation.corporation_id].into_iter().collect(),
        character_ids: vec![character.character_id].into_iter().collect(),
    };

    // Execute
    let result = service.find_existing_entity_ids(&unique_ids).await;

    // Assert
    assert!(result.is_ok());
    let table_ids = result.unwrap();

    assert_eq!(table_ids.faction_ids.len(), 1);
    assert_eq!(table_ids.faction_ids[&faction.faction_id], faction.id);

    assert_eq!(table_ids.alliance_ids.len(), 1);
    assert_eq!(table_ids.alliance_ids[&alliance.alliance_id], alliance.id);

    assert_eq!(table_ids.corporation_ids.len(), 1);
    assert_eq!(
        table_ids.corporation_ids[&corporation.corporation_id],
        corporation.id
    );

    assert_eq!(table_ids.character_ids.len(), 1);
    assert_eq!(
        table_ids.character_ids[&character.character_id],
        character.id
    );

    Ok(())
}

/// Expect Ok with empty maps when no entities exist in database
#[tokio::test]
async fn returns_empty_when_no_entities_exist() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let unique_ids = UniqueIds {
        faction_ids: vec![500001, 500002].into_iter().collect(),
        alliance_ids: vec![99000001, 99000002].into_iter().collect(),
        corporation_ids: vec![98000001, 98000002].into_iter().collect(),
        character_ids: vec![2114794365, 2114794366].into_iter().collect(),
    };

    let result = service.find_existing_entity_ids(&unique_ids).await;

    assert!(result.is_ok());
    let table_ids = result.unwrap();

    assert_eq!(table_ids.faction_ids.len(), 0);
    assert_eq!(table_ids.alliance_ids.len(), 0);
    assert_eq!(table_ids.corporation_ids.len(), 0);
    assert_eq!(table_ids.character_ids.len(), 0);

    Ok(())
}

/// Expect Ok with empty maps when input is empty
#[tokio::test]
async fn returns_empty_for_empty_input() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let unique_ids = UniqueIds {
        faction_ids: HashSet::new(),
        alliance_ids: HashSet::new(),
        corporation_ids: HashSet::new(),
        character_ids: HashSet::new(),
    };

    let result = service.find_existing_entity_ids(&unique_ids).await;

    assert!(result.is_ok());
    let table_ids = result.unwrap();

    assert_eq!(table_ids.faction_ids.len(), 0);
    assert_eq!(table_ids.alliance_ids.len(), 0);
    assert_eq!(table_ids.corporation_ids.len(), 0);
    assert_eq!(table_ids.character_ids.len(), 0);

    Ok(())
}

/// Expect Ok with correct mappings when multiple entities of each type exist
#[tokio::test]
async fn returns_mappings_for_multiple_entities() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    // Insert multiple entities of each type
    let faction1 = test.eve().insert_mock_faction(500001).await?;
    let faction2 = test.eve().insert_mock_faction(500002).await?;
    let faction3 = test.eve().insert_mock_faction(500003).await?;

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
    let corporation3 = test
        .eve()
        .insert_mock_corporation(98000003, None, None)
        .await?;

    let character1 = test
        .eve()
        .insert_mock_character(2114794365, 98000001, None, None)
        .await?;
    let character2 = test
        .eve()
        .insert_mock_character(2114794366, 98000001, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let unique_ids = UniqueIds {
        faction_ids: vec![
            faction1.faction_id,
            faction2.faction_id,
            faction3.faction_id,
        ]
        .into_iter()
        .collect(),
        alliance_ids: vec![alliance1.alliance_id, alliance2.alliance_id]
            .into_iter()
            .collect(),
        corporation_ids: vec![
            corporation1.corporation_id,
            corporation2.corporation_id,
            corporation3.corporation_id,
        ]
        .into_iter()
        .collect(),
        character_ids: vec![character1.character_id, character2.character_id]
            .into_iter()
            .collect(),
    };

    let result = service.find_existing_entity_ids(&unique_ids).await;

    assert!(result.is_ok());
    let table_ids = result.unwrap();

    // Verify correct counts
    assert_eq!(table_ids.faction_ids.len(), 3);
    assert_eq!(table_ids.alliance_ids.len(), 2);
    assert_eq!(table_ids.corporation_ids.len(), 3);
    assert_eq!(table_ids.character_ids.len(), 2);

    // Verify correct mappings
    assert_eq!(table_ids.faction_ids[&faction1.faction_id], faction1.id);
    assert_eq!(table_ids.faction_ids[&faction2.faction_id], faction2.id);
    assert_eq!(table_ids.faction_ids[&faction3.faction_id], faction3.id);

    assert_eq!(table_ids.alliance_ids[&alliance1.alliance_id], alliance1.id);
    assert_eq!(table_ids.alliance_ids[&alliance2.alliance_id], alliance2.id);

    assert_eq!(
        table_ids.corporation_ids[&corporation1.corporation_id],
        corporation1.id
    );
    assert_eq!(
        table_ids.corporation_ids[&corporation2.corporation_id],
        corporation2.id
    );
    assert_eq!(
        table_ids.corporation_ids[&corporation3.corporation_id],
        corporation3.id
    );

    assert_eq!(
        table_ids.character_ids[&character1.character_id],
        character1.id
    );
    assert_eq!(
        table_ids.character_ids[&character2.character_id],
        character2.id
    );

    Ok(())
}

/// Expect Ok with partial results when only some entities exist
#[tokio::test]
async fn returns_partial_results_for_mixed_input() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    // Insert only some entities
    let faction = test.eve().insert_mock_faction(500001).await?;
    let corporation = test
        .eve()
        .insert_mock_corporation(98000001, None, None)
        .await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let unique_ids = UniqueIds {
        faction_ids: vec![500001, 500002, 500003].into_iter().collect(), // Only 500001 exists
        alliance_ids: vec![99000001, 99000002].into_iter().collect(),    // None exist
        corporation_ids: vec![98000001, 98000002].into_iter().collect(), // Only 98000001 exists
        character_ids: vec![2114794365].into_iter().collect(),           // None exist
    };

    let result = service.find_existing_entity_ids(&unique_ids).await;

    assert!(result.is_ok());
    let table_ids = result.unwrap();

    // Should only return the entities that exist
    assert_eq!(table_ids.faction_ids.len(), 1);
    assert_eq!(table_ids.faction_ids[&faction.faction_id], faction.id);

    assert_eq!(table_ids.alliance_ids.len(), 0);

    assert_eq!(table_ids.corporation_ids.len(), 1);
    assert_eq!(
        table_ids.corporation_ids[&corporation.corporation_id],
        corporation.id
    );

    assert_eq!(table_ids.character_ids.len(), 0);

    Ok(())
}

/// Expect Error when required tables haven't been created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?; // No tables created

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let unique_ids = UniqueIds {
        faction_ids: vec![500001].into_iter().collect(),
        alliance_ids: HashSet::new(),
        corporation_ids: HashSet::new(),
        character_ids: HashSet::new(),
    };

    let result = service.find_existing_entity_ids(&unique_ids).await;

    assert!(result.is_err());

    Ok(())
}

/// Expect Ok with mappings for only the requested entity type
#[tokio::test]
async fn returns_mappings_for_single_entity_type() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let faction = test.eve().insert_mock_faction(500001).await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let unique_ids = UniqueIds {
        faction_ids: vec![faction.faction_id].into_iter().collect(),
        alliance_ids: HashSet::new(),
        corporation_ids: HashSet::new(),
        character_ids: HashSet::new(),
    };

    let result = service.find_existing_entity_ids(&unique_ids).await;

    assert!(result.is_ok());
    let table_ids = result.unwrap();

    assert_eq!(table_ids.faction_ids.len(), 1);
    assert_eq!(table_ids.faction_ids[&faction.faction_id], faction.id);
    assert_eq!(table_ids.alliance_ids.len(), 0);
    assert_eq!(table_ids.corporation_ids.len(), 0);
    assert_eq!(table_ids.character_ids.len(), 0);

    Ok(())
}

/// Expect Ok with correct mapping direction from EVE ID to table ID
#[tokio::test]
async fn returns_correct_mapping_direction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let faction = test.eve().insert_mock_faction(500001).await?;

    let service = AffiliationService::new(test.state.db.clone(), test.state.esi_client.clone());

    let unique_ids = UniqueIds {
        faction_ids: vec![faction.faction_id].into_iter().collect(),
        alliance_ids: HashSet::new(),
        corporation_ids: HashSet::new(),
        character_ids: HashSet::new(),
    };

    let result = service.find_existing_entity_ids(&unique_ids).await;

    assert!(result.is_ok());
    let table_ids = result.unwrap();

    // The HashMap should map from EVE ID (i64) to table ID (i32)
    // faction.faction_id is the EVE ID
    // faction.id is the table ID
    assert!(table_ids.faction_ids.contains_key(&faction.faction_id));
    assert_eq!(table_ids.faction_ids[&faction.faction_id], faction.id);

    Ok(())
}
