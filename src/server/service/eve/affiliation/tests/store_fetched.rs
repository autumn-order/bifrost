use super::*;

mod attempt_update_missing_factions {
    use super::*;

    /// Expect Ok when no factions are missing
    #[tokio::test]
    async fn returns_ok_when_no_factions_missing() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let faction = test.eve().insert_mock_faction(500001).await?;

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let mut table_ids = TableIds {
            faction_ids: vec![(faction.faction_id, faction.id)].into_iter().collect(),
            alliance_ids: HashMap::new(),
            corporation_ids: HashMap::new(),
            character_ids: HashMap::new(),
        };

        let mut unique_ids = UniqueIds {
            faction_ids: vec![faction.faction_id].into_iter().collect(),
            alliance_ids: HashSet::new(),
            corporation_ids: HashSet::new(),
            character_ids: HashSet::new(),
        };

        let result = service
            .attempt_update_missing_factions(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());

        Ok(())
    }

    /// Expect Ok when input is empty
    #[tokio::test]
    async fn returns_ok_for_empty_input() -> Result<(), TestError> {
        let test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

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
            corporation_ids: HashSet::new(),
            character_ids: HashSet::new(),
        };

        let result = service
            .attempt_update_missing_factions(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());

        Ok(())
    }

    /// Expect Ok and verify table_ids are updated when factions are fetched
    #[tokio::test]
    async fn updates_table_ids_when_factions_fetched() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let mock_faction = test.eve().with_mock_faction(500001);
        let _faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let mut table_ids = TableIds {
            faction_ids: HashMap::new(),
            alliance_ids: HashMap::new(),
            corporation_ids: HashMap::new(),
            character_ids: HashMap::new(),
        };

        let mut unique_ids = UniqueIds {
            faction_ids: vec![500001].into_iter().collect(),
            alliance_ids: HashSet::new(),
            corporation_ids: HashSet::new(),
            character_ids: HashSet::new(),
        };

        let result = service
            .attempt_update_missing_factions(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        assert_eq!(table_ids.faction_ids.len(), 1);
        assert!(table_ids.faction_ids.contains_key(&500001));

        Ok(())
    }
}

mod store_fetched_characters {
    use super::*;

    /// Expect Ok when storing fetched characters with valid corporation references
    #[tokio::test]
    async fn stores_characters_with_valid_corporation() -> Result<(), TestError> {
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

        let (character_id, character) = test
            .eve()
            .with_mock_character(2114794365, 98000001, None, None);

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let table_ids = TableIds {
            faction_ids: HashMap::new(),
            alliance_ids: HashMap::new(),
            corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
            character_ids: HashMap::new(),
        };

        let fetched_characters = vec![(character_id, character)];

        let result = service
            .store_fetched_characters(fetched_characters, &table_ids)
            .await;

        assert!(result.is_ok());

        Ok(())
    }

    /// Expect Ok but skip characters when corporation reference is missing
    #[tokio::test]
    async fn skips_characters_with_missing_corporation() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let (character_id, character) = test
            .eve()
            .with_mock_character(2114794365, 98000001, None, None);

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let table_ids = TableIds {
            faction_ids: HashMap::new(),
            alliance_ids: HashMap::new(),
            corporation_ids: HashMap::new(), // No corporation mapping
            character_ids: HashMap::new(),
        };

        let fetched_characters = vec![(character_id, character)];

        let result = service
            .store_fetched_characters(fetched_characters, &table_ids)
            .await;

        assert!(result.is_ok());

        Ok(())
    }

    /// Expect Ok when storing characters with faction references
    #[tokio::test]
    async fn stores_characters_with_faction_reference() -> Result<(), TestError> {
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

        let (character_id, character) =
            test.eve()
                .with_mock_character(2114794365, 98000001, None, Some(500001));

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let table_ids = TableIds {
            faction_ids: vec![(500001, faction.id)].into_iter().collect(),
            alliance_ids: HashMap::new(),
            corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
            character_ids: HashMap::new(),
        };

        let fetched_characters = vec![(character_id, character)];

        let result = service
            .store_fetched_characters(fetched_characters, &table_ids)
            .await;

        assert!(result.is_ok());

        Ok(())
    }
}

mod store_fetched_corporations {
    use super::*;

    /// Expect Ok when storing fetched corporations
    #[tokio::test]
    async fn stores_corporations_and_updates_table_ids() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let (corporation_id, corporation) = test.eve().with_mock_corporation(98000001, None, None);

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let mut table_ids = TableIds {
            faction_ids: HashMap::new(),
            alliance_ids: HashMap::new(),
            corporation_ids: HashMap::new(),
            character_ids: HashMap::new(),
        };

        let fetched_corporations = vec![(corporation_id, corporation)];

        let result = service
            .store_fetched_corporations(fetched_corporations, &mut table_ids)
            .await;

        assert!(result.is_ok());
        assert_eq!(table_ids.corporation_ids.len(), 1);
        assert!(table_ids.corporation_ids.contains_key(&corporation_id));

        Ok(())
    }

    /// Expect Ok when storing corporations with alliance references
    #[tokio::test]
    async fn stores_corporations_with_alliance_reference() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let alliance = test.eve().insert_mock_alliance(99000001, None).await?;

        let (corporation_id, corporation) =
            test.eve()
                .with_mock_corporation(98000001, Some(99000001), None);

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let mut table_ids = TableIds {
            faction_ids: HashMap::new(),
            alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
            corporation_ids: HashMap::new(),
            character_ids: HashMap::new(),
        };

        let fetched_corporations = vec![(corporation_id, corporation)];

        let result = service
            .store_fetched_corporations(fetched_corporations, &mut table_ids)
            .await;

        assert!(result.is_ok());
        assert!(table_ids.corporation_ids.contains_key(&corporation_id));

        Ok(())
    }

    /// Expect Ok when storing corporations with faction references
    #[tokio::test]
    async fn stores_corporations_with_faction_reference() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let faction = test.eve().insert_mock_faction(500001).await?;

        let (corporation_id, corporation) =
            test.eve()
                .with_mock_corporation(98000001, None, Some(500001));

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let mut table_ids = TableIds {
            faction_ids: vec![(500001, faction.id)].into_iter().collect(),
            alliance_ids: HashMap::new(),
            corporation_ids: HashMap::new(),
            character_ids: HashMap::new(),
        };

        let fetched_corporations = vec![(corporation_id, corporation)];

        let result = service
            .store_fetched_corporations(fetched_corporations, &mut table_ids)
            .await;

        assert!(result.is_ok());
        assert!(table_ids.corporation_ids.contains_key(&corporation_id));

        Ok(())
    }
}

mod store_fetched_alliances {
    use super::*;

    /// Expect Ok when storing fetched alliances
    #[tokio::test]
    async fn stores_alliances_and_updates_table_ids() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let (alliance_id, alliance) = test.eve().with_mock_alliance(99000001, None);

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let mut table_ids = TableIds {
            faction_ids: HashMap::new(),
            alliance_ids: HashMap::new(),
            corporation_ids: HashMap::new(),
            character_ids: HashMap::new(),
        };

        let fetched_alliances = vec![(alliance_id, alliance)];

        let result = service
            .store_fetched_alliances(fetched_alliances, &mut table_ids)
            .await;

        assert!(result.is_ok());
        assert_eq!(table_ids.alliance_ids.len(), 1);
        assert!(table_ids.alliance_ids.contains_key(&alliance_id));

        Ok(())
    }

    /// Expect Ok when storing alliances with faction references
    #[tokio::test]
    async fn stores_alliances_with_faction_reference() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let faction = test.eve().insert_mock_faction(500001).await?;

        let (alliance_id, alliance) = test.eve().with_mock_alliance(99000001, Some(500001));

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let mut table_ids = TableIds {
            faction_ids: vec![(500001, faction.id)].into_iter().collect(),
            alliance_ids: HashMap::new(),
            corporation_ids: HashMap::new(),
            character_ids: HashMap::new(),
        };

        let fetched_alliances = vec![(alliance_id, alliance)];

        let result = service
            .store_fetched_alliances(fetched_alliances, &mut table_ids)
            .await;

        assert!(result.is_ok());
        assert!(table_ids.alliance_ids.contains_key(&alliance_id));

        Ok(())
    }
}
