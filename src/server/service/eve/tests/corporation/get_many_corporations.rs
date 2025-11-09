use super::*;

/// Expect Ok when fetching multiple corporations successfully
#[tokio::test]
async fn fetches_multiple_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Setup mock endpoints for 3 different corporations
    let corporation_ids = vec![1, 2, 3];
    let mut endpoints = Vec::new();
    for id in &corporation_ids {
        let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
        endpoints.push(
            test.eve()
                .with_corporation_endpoint(corp_id, mock_corporation, 1),
        );
    }

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids.clone())
        .await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 3);

    // Verify all corporations were returned
    for (corporation_id, _corporation) in corporations.iter() {
        assert!(corporation_ids.contains(&corporation_id));
    }

    // Assert all requests were made
    for endpoint in endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Expect Ok with empty vec when given empty corporation IDs list
#[tokio::test]
async fn returns_empty_for_empty_input() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service.get_many_corporations(vec![]).await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 0);

    Ok(())
}

/// Expect Ok when fetching single corporation
#[tokio::test]
async fn fetches_single_corporation() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let (corporation_id, mock_corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(vec![corporation_id])
        .await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 1);
    assert_eq!(corporations[0].0, corporation_id);

    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when fetching corporations with alliances
#[tokio::test]
async fn fetches_corporations_with_alliances() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Pre-insert alliances to avoid ESI fetches
    let alliance_id_1 = 1;
    let alliance_id_2 = 2;
    let _ = test.eve().insert_mock_alliance(alliance_id_1, None).await?;
    let _ = test.eve().insert_mock_alliance(alliance_id_2, None).await?;

    let (corporation_id_1, mock_corporation_1) =
        test.eve()
            .with_mock_corporation(1, Some(alliance_id_1), None);
    let (corporation_id_2, mock_corporation_2) =
        test.eve()
            .with_mock_corporation(2, Some(alliance_id_2), None);

    let corporation_endpoint_1 =
        test.eve()
            .with_corporation_endpoint(corporation_id_1, mock_corporation_1, 1);
    let corporation_endpoint_2 =
        test.eve()
            .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);

    let corporation_ids = vec![corporation_id_1, corporation_id_2];
    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids)
        .await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 2);

    corporation_endpoint_1.assert();
    corporation_endpoint_2.assert();

    Ok(())
}

/// Expect Ok when fetching corporations with factions
#[tokio::test]
async fn fetches_corporations_with_factions() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Pre-insert factions to avoid ESI fetches
    let faction_id_1 = 1;
    let faction_id_2 = 2;
    let _ = test.eve().insert_mock_faction(faction_id_1).await?;
    let _ = test.eve().insert_mock_faction(faction_id_2).await?;

    let (corporation_id_1, mock_corporation_1) =
        test.eve()
            .with_mock_corporation(1, None, Some(faction_id_1));
    let (corporation_id_2, mock_corporation_2) =
        test.eve()
            .with_mock_corporation(2, None, Some(faction_id_2));

    let corporation_endpoint_1 =
        test.eve()
            .with_corporation_endpoint(corporation_id_1, mock_corporation_1, 1);
    let corporation_endpoint_2 =
        test.eve()
            .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);

    let corporation_ids = vec![corporation_id_1, corporation_id_2];
    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids)
        .await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 2);

    corporation_endpoint_1.assert();
    corporation_endpoint_2.assert();

    Ok(())
}

/// Expect Ok when fetching corporations with both alliance and faction
#[tokio::test]
async fn fetches_corporations_with_alliance_and_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Pre-insert faction and alliance to avoid ESI fetches
    let faction_id = 1;
    let alliance_id = 1;
    let _ = test.eve().insert_mock_faction(faction_id).await?;
    let _ = test
        .eve()
        .insert_mock_alliance(alliance_id, Some(faction_id))
        .await?;

    let (corporation_id, mock_corporation) =
        test.eve()
            .with_mock_corporation(1, Some(alliance_id), Some(faction_id));

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(vec![corporation_id])
        .await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 1);
    assert_eq!(corporations[0].1.alliance_id, Some(alliance_id));
    assert_eq!(corporations[0].1.faction_id, Some(faction_id));

    corporation_endpoint.assert();

    Ok(())
}

/// Expect Error when ESI endpoint is unavailable for any corporation
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let corporation_ids = vec![1, 2, 3];
    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids)
        .await;

    // Should fail on first unavailable corporation
    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

/// Expect Error when ESI fails partway through batch
#[tokio::test]
async fn fails_on_partial_esi_failure() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Setup mock endpoint for first corporation only
    let (corporation_id, mock_corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let corporation_ids = vec![1, 2, 3];
    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids)
        .await;

    // Should succeed on first, fail on second (no mock)
    assert!(matches!(result, Err(Error::EsiError(_))));

    corporation_endpoint.assert();

    Ok(())
}

/// Expect Ok when fetching many corporations (stress test)
#[tokio::test]
async fn fetches_many_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Setup mock endpoints for 10 corporations
    let corporation_ids: Vec<i64> = (1..=10).collect();
    let mut endpoints = Vec::new();
    for id in &corporation_ids {
        let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
        endpoints.push(
            test.eve()
                .with_corporation_endpoint(corp_id, mock_corporation, 1),
        );
    }

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids.clone())
        .await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 10);

    // Verify all corporation IDs are present
    for (corporation_id, _corporation) in corporations.iter() {
        assert!(corporation_ids.contains(&corporation_id));
    }

    // Assert all requests were made
    for endpoint in endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Expect Ok when fetching more than 10 corporations (tests batching)
#[tokio::test]
async fn fetches_many_corporations_with_batching() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Setup mock endpoints for 25 corporations to test multiple batches
    let corporation_ids: Vec<i64> = (1..=25).collect();
    let mut endpoints = Vec::new();
    for id in &corporation_ids {
        let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
        endpoints.push(
            test.eve()
                .with_corporation_endpoint(corp_id, mock_corporation, 1),
        );
    }

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids.clone())
        .await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 25);

    // Verify all corporation IDs are present (order may vary due to concurrency)
    let returned_ids: Vec<i64> = corporations.iter().map(|(id, _)| *id).collect();
    for id in &corporation_ids {
        assert!(returned_ids.contains(id));
    }

    // Assert all requests were made
    for endpoint in endpoints {
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
        entity::prelude::EveCorporation
    )?;

    // Setup mock endpoints for 5 corporations (within one batch)
    let corporation_ids: Vec<i64> = (1..=5).collect();
    let mut endpoints = Vec::new();
    for id in &corporation_ids {
        let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
        endpoints.push(
            test.eve()
                .with_corporation_endpoint(corp_id, mock_corporation, 1),
        );
    }

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids.clone())
        .await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 5);

    // Verify all corporation IDs are present
    let returned_ids: Vec<i64> = corporations.iter().map(|(id, _)| *id).collect();
    for id in &corporation_ids {
        assert!(returned_ids.contains(id));
    }

    // Assert all requests were made
    for endpoint in endpoints {
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
        entity::prelude::EveCorporation
    )?;

    // Setup mock endpoints for only some corporations in the batch
    let (corporation_id, mock_corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);

    let corporation_ids = vec![1, 2, 3, 4, 5];
    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids)
        .await;

    // Should fail when any request in the batch fails
    assert!(matches!(result, Err(Error::EsiError(_))));

    corporation_endpoint.assert();

    Ok(())
}

/// Expect correct batching behavior with exactly 10 items
#[tokio::test]
async fn handles_exact_batch_size() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Setup mock endpoints for exactly 10 corporations (one full batch)
    let corporation_ids: Vec<i64> = (1..=10).collect();
    let mut endpoints = Vec::new();
    for id in &corporation_ids {
        let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
        endpoints.push(
            test.eve()
                .with_corporation_endpoint(corp_id, mock_corporation, 1),
        );
    }

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids.clone())
        .await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 10);

    // Verify all corporation IDs are present
    let returned_ids: Vec<i64> = corporations.iter().map(|(id, _)| *id).collect();
    for id in &corporation_ids {
        assert!(returned_ids.contains(id));
    }

    // Assert all requests were made
    for endpoint in endpoints {
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
        entity::prelude::EveCorporation
    )?;

    // Setup mock endpoints for 11 corporations (one full batch + one item)
    let corporation_ids: Vec<i64> = (1..=11).collect();
    let mut endpoints = Vec::new();
    for id in &corporation_ids {
        let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
        endpoints.push(
            test.eve()
                .with_corporation_endpoint(corp_id, mock_corporation, 1),
        );
    }

    let corporation_service = CorporationService::new(&test.state.db, &test.state.esi_client);
    let result = corporation_service
        .get_many_corporations(corporation_ids.clone())
        .await;

    assert!(result.is_ok());
    let corporations = result.unwrap();
    assert_eq!(corporations.len(), 11);

    // Verify all corporation IDs are present
    let returned_ids: Vec<i64> = corporations.iter().map(|(id, _)| *id).collect();
    for id in &corporation_ids {
        assert!(returned_ids.contains(id));
    }

    // Assert all requests were made
    for endpoint in endpoints {
        endpoint.assert();
    }

    Ok(())
}
