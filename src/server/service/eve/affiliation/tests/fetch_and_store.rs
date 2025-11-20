use super::*;

/// Expect Ok when all entities are already present
#[tokio::test]
async fn returns_ok_when_no_entities_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

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

    let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

    let mut table_ids = TableIds {
        faction_ids: vec![(faction.faction_id, faction.id)].into_iter().collect(),
        alliance_ids: vec![(alliance.alliance_id, alliance.id)]
            .into_iter()
            .collect(),
        corporation_ids: vec![(corporation.corporation_id, corporation.id)]
            .into_iter()
            .collect(),
        character_ids: vec![(character.character_id, character.id)]
            .into_iter()
            .collect(),
    };

    let mut unique_ids = UniqueIds {
        faction_ids: vec![faction.faction_id].into_iter().collect(),
        alliance_ids: vec![alliance.alliance_id].into_iter().collect(),
        corporation_ids: vec![corporation.corporation_id].into_iter().collect(),
        character_ids: vec![character.character_id].into_iter().collect(),
    };

    let result = service
        .fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
        .await;

    assert!(result.is_ok());

    Ok(())
}

/// Expect Ok when fetching and storing missing characters
#[tokio::test]
async fn fetches_and_stores_missing_characters() -> Result<(), TestError> {
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

    let (character_id, mock_character) = test
        .eve()
        .with_mock_character(2114794365, 98000001, None, None);
    let _character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

    let mut table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
        character_ids: HashMap::new(),
    };

    let mut unique_ids = UniqueIds {
        faction_ids: HashSet::new(),
        alliance_ids: HashSet::new(),
        corporation_ids: vec![98000001].into_iter().collect(),
        character_ids: vec![character_id].into_iter().collect(),
    };

    let result = service
        .fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
        .await;

    assert!(result.is_ok());

    Ok(())
}

/// Expect Ok when fetching and storing missing corporations
#[tokio::test]
async fn fetches_and_stores_missing_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let (corporation_id, mock_corporation) = test.eve().with_mock_corporation(98000001, None, None);
    let _corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

    let mut table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: HashMap::new(),
        character_ids: HashMap::new(),
    };

    let mut unique_ids = UniqueIds {
        faction_ids: HashSet::new(),
        alliance_ids: HashSet::new(),
        corporation_ids: vec![corporation_id].into_iter().collect(),
        character_ids: HashSet::new(),
    };

    let result = service
        .fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
        .await;

    assert!(result.is_ok());
    assert!(table_ids.corporation_ids.contains_key(&corporation_id));

    Ok(())
}

/// Expect Ok when fetching and storing missing alliances
#[tokio::test]
async fn fetches_and_stores_missing_alliances() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(99000001, None);
    let _alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

    let mut table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: HashMap::new(),
        character_ids: HashMap::new(),
    };

    let mut unique_ids = UniqueIds {
        faction_ids: HashSet::new(),
        alliance_ids: vec![alliance_id].into_iter().collect(),
        corporation_ids: HashSet::new(),
        character_ids: HashSet::new(),
    };

    let result = service
        .fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
        .await;

    assert!(result.is_ok());
    assert!(table_ids.alliance_ids.contains_key(&alliance_id));

    Ok(())
}

/// Expect Ok when fetching entities with pre-populated dependencies
#[tokio::test]
async fn fetches_entities_with_dependencies() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter,
    )?;

    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(99000001, None);
    let _alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    let (corporation_id, mock_corporation) =
        test.eve()
            .with_mock_corporation(98000001, Some(99000001), None);
    let _corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let (character_id, mock_character) = test
        .eve()
        .with_mock_character(2114794365, 98000001, None, None);
    let _character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

    let mut table_ids = TableIds {
        faction_ids: HashMap::new(),
        alliance_ids: HashMap::new(),
        corporation_ids: HashMap::new(),
        character_ids: HashMap::new(),
    };

    // Pre-populate unique_ids with all IDs to simulate a proper affiliation update
    let mut unique_ids = UniqueIds {
        faction_ids: HashSet::new(),
        alliance_ids: vec![alliance_id].into_iter().collect(),
        corporation_ids: vec![corporation_id].into_iter().collect(),
        character_ids: vec![character_id].into_iter().collect(),
    };

    let result = service
        .fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
        .await;

    assert!(result.is_ok());
    // Verify that entities were stored and added to table_ids
    assert!(table_ids.alliance_ids.contains_key(&alliance_id));
    assert!(table_ids.corporation_ids.contains_key(&corporation_id));

    Ok(())
}
