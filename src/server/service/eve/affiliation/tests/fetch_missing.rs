use super::*;

mod fetch_missing_characters {
    use super::*;

    /// Expect Ok with fetched characters when characters are missing from database
    #[tokio::test]
    async fn fetches_missing_characters_from_esi() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        // Insert corporation so character can reference it
        let corporation = test
            .eve()
            .insert_mock_corporation(98000001, None, None)
            .await?;

        // Setup mock ESI endpoints
        let (character_id, mock_character) = test
            .eve()
            .with_mock_character(2114794365, 98000001, None, None);
        let character_endpoint =
            test.eve()
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
            .fetch_missing_characters(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].0, character_id);

        character_endpoint.assert();

        Ok(())
    }

    /// Expect Ok with empty vec when no characters are missing
    #[tokio::test]
    async fn returns_empty_when_no_characters_missing() -> Result<(), TestError> {
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

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let mut table_ids = TableIds {
            faction_ids: HashMap::new(),
            alliance_ids: HashMap::new(),
            corporation_ids: HashMap::new(),
            character_ids: vec![(character.character_id, character.id)]
                .into_iter()
                .collect(),
        };

        let mut unique_ids = UniqueIds {
            faction_ids: HashSet::new(),
            alliance_ids: HashSet::new(),
            corporation_ids: HashSet::new(),
            character_ids: vec![character.character_id].into_iter().collect(),
        };

        let result = service
            .fetch_missing_characters(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched.len(), 0);

        Ok(())
    }

    /// Expect Ok with empty vec when input is empty
    #[tokio::test]
    async fn returns_empty_for_empty_input() -> Result<(), TestError> {
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
            .fetch_missing_characters(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched.len(), 0);

        Ok(())
    }

    /// Expect Ok and verify corporation_ids are added to unique_ids
    #[tokio::test]
    async fn adds_corporation_ids_to_unique_ids() -> Result<(), TestError> {
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
        let _character_endpoint =
            test.eve()
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
            corporation_ids: HashSet::new(),
            character_ids: vec![character_id].into_iter().collect(),
        };

        let result = service
            .fetch_missing_characters(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        assert!(unique_ids.corporation_ids.contains(&98000001));

        Ok(())
    }

    /// Expect Ok and verify faction_ids are added to unique_ids when present
    #[tokio::test]
    async fn adds_faction_ids_to_unique_ids_when_present() -> Result<(), TestError> {
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

        let (character_id, mock_character) =
            test.eve()
                .with_mock_character(2114794365, 98000001, None, Some(500001));
        let _character_endpoint =
            test.eve()
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
            corporation_ids: HashSet::new(),
            character_ids: vec![character_id].into_iter().collect(),
        };

        let result = service
            .fetch_missing_characters(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        assert!(unique_ids.faction_ids.contains(&500001));

        Ok(())
    }
}

mod fetch_missing_corporations {
    use super::*;

    /// Expect Ok with fetched corporations when corporations are missing from database
    #[tokio::test]
    async fn fetches_missing_corporations_from_esi() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let (corporation_id, mock_corporation) =
            test.eve().with_mock_corporation(98000001, None, None);
        let corporation_endpoint =
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
            .fetch_missing_corporations(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].0, corporation_id);

        corporation_endpoint.assert();

        Ok(())
    }

    /// Expect Ok with empty vec when no corporations are missing
    #[tokio::test]
    async fn returns_empty_when_no_corporations_missing() -> Result<(), TestError> {
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

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let mut table_ids = TableIds {
            faction_ids: HashMap::new(),
            alliance_ids: HashMap::new(),
            corporation_ids: vec![(corporation.corporation_id, corporation.id)]
                .into_iter()
                .collect(),
            character_ids: HashMap::new(),
        };

        let mut unique_ids = UniqueIds {
            faction_ids: HashSet::new(),
            alliance_ids: HashSet::new(),
            corporation_ids: vec![corporation.corporation_id].into_iter().collect(),
            character_ids: HashSet::new(),
        };

        let result = service
            .fetch_missing_corporations(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched.len(), 0);

        Ok(())
    }

    /// Expect Ok with empty vec when input is empty
    #[tokio::test]
    async fn returns_empty_for_empty_input() -> Result<(), TestError> {
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
            .fetch_missing_corporations(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched.len(), 0);

        Ok(())
    }

    /// Expect Ok and verify alliance_ids are added to unique_ids when present
    #[tokio::test]
    async fn adds_alliance_ids_to_unique_ids_when_present() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let (corporation_id, mock_corporation) =
            test.eve()
                .with_mock_corporation(98000001, Some(99000001), None);
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
            .fetch_missing_corporations(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        assert!(unique_ids.alliance_ids.contains(&99000001));

        Ok(())
    }

    /// Expect Ok and verify faction_ids are added to unique_ids when present
    #[tokio::test]
    async fn adds_faction_ids_to_unique_ids_when_present() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let (corporation_id, mock_corporation) =
            test.eve()
                .with_mock_corporation(98000001, None, Some(500001));
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
            .fetch_missing_corporations(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        assert!(unique_ids.faction_ids.contains(&500001));

        Ok(())
    }
}

mod fetch_missing_alliances {
    use super::*;

    /// Expect Ok with fetched alliances when alliances are missing from database
    #[tokio::test]
    async fn fetches_missing_alliances_from_esi() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(99000001, None);
        let alliance_endpoint = test
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
            .fetch_missing_alliances(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].0, alliance_id);

        alliance_endpoint.assert();

        Ok(())
    }

    /// Expect Ok with empty vec when no alliances are missing
    #[tokio::test]
    async fn returns_empty_when_no_alliances_missing() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let alliance = test.eve().insert_mock_alliance(99000001, None).await?;

        let service = AffiliationService::new(&test.state.db, &test.state.esi_client);

        let mut table_ids = TableIds {
            faction_ids: HashMap::new(),
            alliance_ids: vec![(alliance.alliance_id, alliance.id)]
                .into_iter()
                .collect(),
            corporation_ids: HashMap::new(),
            character_ids: HashMap::new(),
        };

        let mut unique_ids = UniqueIds {
            faction_ids: HashSet::new(),
            alliance_ids: vec![alliance.alliance_id].into_iter().collect(),
            corporation_ids: HashSet::new(),
            character_ids: HashSet::new(),
        };

        let result = service
            .fetch_missing_alliances(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched.len(), 0);

        Ok(())
    }

    /// Expect Ok with empty vec when input is empty
    #[tokio::test]
    async fn returns_empty_for_empty_input() -> Result<(), TestError> {
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
            .fetch_missing_alliances(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched.len(), 0);

        Ok(())
    }

    /// Expect Ok and verify faction_ids are added to unique_ids when present
    #[tokio::test]
    async fn adds_faction_ids_to_unique_ids_when_present() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!(
            entity::prelude::EveFaction,
            entity::prelude::EveAlliance,
            entity::prelude::EveCorporation,
            entity::prelude::EveCharacter,
        )?;

        let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(99000001, Some(500001));
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
            .fetch_missing_alliances(&mut table_ids, &mut unique_ids)
            .await;

        assert!(result.is_ok());
        assert!(unique_ids.faction_ids.contains(&500001));

        Ok(())
    }
}
