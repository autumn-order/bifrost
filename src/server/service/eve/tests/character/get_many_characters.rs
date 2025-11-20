use super::*;

/// Expect Ok when fetching multiple characters successfully
#[tokio::test]
async fn fetches_multiple_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Setup mock endpoints for 3 different characters
    // Pre-insert the shared corporation to avoid ESI fetch
    let corporation_id = 1;
    let _ = test
        .eve()
        .insert_mock_corporation(corporation_id, None, None)
        .await?;

    let character_ids = vec![1, 2, 3];
    let mut character_endpoints = Vec::new();
    for id in &character_ids {
        let (char_id, mock_character) =
            test.eve()
                .with_mock_character(*id, corporation_id, None, None);
        character_endpoints.push(
            test.eve()
                .with_character_endpoint(char_id, mock_character, 1),
        );
    }

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_many_characters(character_ids.clone())
        .await;

    assert!(result.is_ok());
    let characters = result.unwrap();
    assert_eq!(characters.len(), 3);

    // Verify all character IDs are present (order may vary due to concurrency)
    let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
    for id in &character_ids {
        assert!(returned_ids.contains(id));
    }

    for endpoint in character_endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Expect Ok with empty vec when given empty character IDs list
#[tokio::test]
async fn returns_empty_for_empty_input() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service.get_many_characters(vec![]).await;

    assert!(result.is_ok());
    let characters = result.unwrap();
    assert_eq!(characters.len(), 0);

    Ok(())
}

/// Expect Ok when fetching single character
#[tokio::test]
async fn fetches_single_character() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Pre-insert the corporation to avoid ESI fetch
    let corporation_id = 1;
    let _ = test
        .eve()
        .insert_mock_corporation(corporation_id, None, None)
        .await?;

    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(1, corporation_id, None, None);

    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_many_characters(vec![character_id])
        .await;

    assert!(result.is_ok());
    let characters = result.unwrap();
    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].0, character_id);

    character_endpoint.assert();

    Ok(())
}

/// Expect Ok when fetching characters with various relationships
#[tokio::test]
async fn fetches_characters_with_relationships() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let faction_id = 1;
    let alliance_id = 1;
    let corporation_id = 1;

    // Pre-insert dependencies to avoid ESI fetches
    let _ = test.eve().insert_mock_faction(faction_id).await?;
    let _ = test
        .eve()
        .insert_mock_alliance(alliance_id, Some(faction_id))
        .await?;
    let _ = test
        .eve()
        .insert_mock_corporation(corporation_id, Some(alliance_id), Some(faction_id))
        .await?;

    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(1, corporation_id, Some(alliance_id), Some(faction_id));

    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_many_characters(vec![character_id])
        .await;

    assert!(result.is_ok());
    let characters = result.unwrap();
    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].1.faction_id, Some(faction_id));
    assert_eq!(characters[0].1.alliance_id, Some(alliance_id));

    character_endpoint.assert();

    Ok(())
}

/// Expect Error when ESI endpoint is unavailable for any character
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    let character_ids = vec![1, 2, 3];
    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service.get_many_characters(character_ids).await;

    // Should fail on first unavailable character
    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

/// Expect Error when ESI fails partway through batch
#[tokio::test]
async fn fails_on_partial_esi_failure() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Setup mock endpoint for first character only
    // Pre-insert the corporation to avoid ESI fetch
    let corporation_id = 1;
    let _ = test
        .eve()
        .insert_mock_corporation(corporation_id, None, None)
        .await?;

    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(1, corporation_id, None, None);

    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let character_ids = vec![1, 2, 3];
    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service.get_many_characters(character_ids).await;

    // Should succeed on first, fail on second (no mock)
    assert!(matches!(result, Err(Error::EsiError(_))));

    character_endpoint.assert();

    Ok(())
}

/// Expect Ok when fetching many characters (stress test)
#[tokio::test]
async fn fetches_many_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Setup mock endpoints for 10 characters
    // Pre-insert the shared corporation to avoid ESI fetch
    let corporation_id = 1;
    let _ = test
        .eve()
        .insert_mock_corporation(corporation_id, None, None)
        .await?;

    let character_ids: Vec<i64> = (1..=10).collect();
    let mut character_endpoints = Vec::new();
    for id in &character_ids {
        let (char_id, mock_character) =
            test.eve()
                .with_mock_character(*id, corporation_id, None, None);
        character_endpoints.push(
            test.eve()
                .with_character_endpoint(char_id, mock_character, 1),
        );
    }

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_many_characters(character_ids.clone())
        .await;

    assert!(result.is_ok());
    let characters = result.unwrap();
    assert_eq!(characters.len(), 10);

    // Verify all character IDs are present (order may vary due to concurrency)
    let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
    for id in &character_ids {
        assert!(returned_ids.contains(id));
    }

    for endpoint in character_endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Expect Ok when fetching more than 10 characters (tests batching)
#[tokio::test]
async fn fetches_many_characters_with_batching() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Setup mock endpoints for 25 characters to test multiple batches
    // Pre-insert the shared corporation to avoid ESI fetch
    let corporation_id = 1;
    let _ = test
        .eve()
        .insert_mock_corporation(corporation_id, None, None)
        .await?;

    let character_ids: Vec<i64> = (1..=25).collect();
    let mut character_endpoints = Vec::new();
    for id in &character_ids {
        let (char_id, mock_character) =
            test.eve()
                .with_mock_character(*id, corporation_id, None, None);
        character_endpoints.push(
            test.eve()
                .with_character_endpoint(char_id, mock_character, 1),
        );
    }

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_many_characters(character_ids.clone())
        .await;

    assert!(result.is_ok());
    let characters = result.unwrap();
    assert_eq!(characters.len(), 25);

    // Verify all character IDs are present (order may vary due to concurrency)
    let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
    for id in &character_ids {
        assert!(returned_ids.contains(id));
    }

    for endpoint in character_endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Expect Ok when verifying concurrent execution within a batch
#[tokio::test]
async fn executes_requests_concurrently() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Setup mock endpoints for 5 characters (within one batch)
    // Pre-insert the shared corporation to avoid ESI fetch
    let corporation_id = 1;
    let _ = test
        .eve()
        .insert_mock_corporation(corporation_id, None, None)
        .await?;

    let character_ids: Vec<i64> = (1..=5).collect();
    let mut character_endpoints = Vec::new();
    for id in &character_ids {
        let (char_id, mock_character) =
            test.eve()
                .with_mock_character(*id, corporation_id, None, None);
        character_endpoints.push(
            test.eve()
                .with_character_endpoint(char_id, mock_character, 1),
        );
    }

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_many_characters(character_ids.clone())
        .await;

    assert!(result.is_ok());
    let characters = result.unwrap();
    assert_eq!(characters.len(), 5);

    // Verify all character IDs are present
    let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
    for id in &character_ids {
        assert!(returned_ids.contains(id));
    }

    for endpoint in character_endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Expect Error when ESI fails in middle of concurrent batch
#[tokio::test]
async fn fails_on_concurrent_batch_error() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Pre-insert the corporation to avoid ESI fetch
    let corporation_id = 1;
    let _ = test
        .eve()
        .insert_mock_corporation(corporation_id, None, None)
        .await?;

    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(1, corporation_id, None, None);

    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    let character_ids = vec![1, 2, 3, 4, 5];
    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service.get_many_characters(character_ids).await;

    // Should fail when any request in the batch fails
    assert!(matches!(result, Err(Error::EsiError(_))));

    character_endpoint.assert();

    Ok(())
}

/// Expect correct batching behavior with exactly 10 items
#[tokio::test]
async fn handles_exact_batch_size() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Setup mock endpoints for exactly 10 characters (one full batch)
    // Pre-insert the shared corporation to avoid ESI fetch
    let corporation_id = 1;
    let _ = test
        .eve()
        .insert_mock_corporation(corporation_id, None, None)
        .await?;

    let character_ids: Vec<i64> = (1..=10).collect();
    let mut character_endpoints = Vec::new();
    for id in &character_ids {
        let (char_id, mock_character) =
            test.eve()
                .with_mock_character(*id, corporation_id, None, None);
        character_endpoints.push(
            test.eve()
                .with_character_endpoint(char_id, mock_character, 1),
        );
    }

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_many_characters(character_ids.clone())
        .await;

    assert!(result.is_ok());
    let characters = result.unwrap();
    assert_eq!(characters.len(), 10);

    // Verify all character IDs are present
    let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
    for id in &character_ids {
        assert!(returned_ids.contains(id));
    }

    for endpoint in character_endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Expect correct batching behavior with 11 items (tests partial second batch)
#[tokio::test]
async fn handles_batch_size_plus_one() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation,
        entity::prelude::EveCharacter
    )?;

    // Setup mock endpoints for 11 characters (one full batch + one item)
    // Pre-insert the shared corporation to avoid ESI fetch
    let corporation_id = 1;
    let _ = test
        .eve()
        .insert_mock_corporation(corporation_id, None, None)
        .await?;

    let character_ids: Vec<i64> = (1..=11).collect();
    let mut character_endpoints = Vec::new();
    for id in &character_ids {
        let (char_id, mock_character) =
            test.eve()
                .with_mock_character(*id, corporation_id, None, None);
        character_endpoints.push(
            test.eve()
                .with_character_endpoint(char_id, mock_character, 1),
        );
    }

    let character_service =
        CharacterService::new(test.state.db.clone(), test.state.esi_client.clone());
    let result = character_service
        .get_many_characters(character_ids.clone())
        .await;

    assert!(result.is_ok());
    let characters = result.unwrap();
    assert_eq!(characters.len(), 11);

    // Verify all character IDs are present
    let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
    for id in &character_ids {
        assert!(returned_ids.contains(id));
    }

    for endpoint in character_endpoints {
        endpoint.assert();
    }

    Ok(())
}
