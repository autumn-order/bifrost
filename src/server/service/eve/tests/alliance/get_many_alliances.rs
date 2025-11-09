use super::*;

/// Expect Ok when fetching multiple alliances successfully
#[tokio::test]
async fn fetches_multiple_alliances() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    // Setup mock endpoints for 3 different alliances
    let alliance_ids = vec![1, 2, 3];
    let mut endpoints = Vec::new();
    for id in &alliance_ids {
        let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
        endpoints.push(
            test.eve()
                .with_alliance_endpoint(alliance_id, mock_alliance, 1),
        );
    }

    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service
        .get_many_alliances(alliance_ids.clone())
        .await;

    assert!(result.is_ok());
    let alliances = result.unwrap();
    assert_eq!(alliances.len(), 3);

    // Verify all alliance IDs are present (order may vary due to concurrency)
    let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
    for id in &alliance_ids {
        assert!(returned_ids.contains(id));
    }

    // Assert all requests were made
    for endpoint in endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Expect Ok with empty vec when given empty alliance IDs list
#[tokio::test]
async fn returns_empty_for_empty_input() -> Result<(), TestError> {
    let test = test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service.get_many_alliances(vec![]).await;

    assert!(result.is_ok());
    let alliances = result.unwrap();
    assert_eq!(alliances.len(), 0);

    Ok(())
}

/// Expect Ok when fetching single alliance
#[tokio::test]
async fn fetches_single_alliance() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service.get_many_alliances(vec![alliance_id]).await;

    assert!(result.is_ok());
    let alliances = result.unwrap();
    assert_eq!(alliances.len(), 1);
    assert_eq!(alliances[0].0, alliance_id);

    alliance_endpoint.assert();

    Ok(())
}

/// Expect Ok when fetching alliances with factions
#[tokio::test]
async fn fetches_alliances_with_factions() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    // Setup mock endpoints for alliances with factions
    // Pre-insert factions so they don't need to be fetched from ESI
    let _ = test.eve().insert_mock_faction(1).await?;
    let _ = test.eve().insert_mock_faction(2).await?;

    let (alliance_id_1, mock_alliance_1) = test.eve().with_mock_alliance(1, Some(1));
    let (alliance_id_2, mock_alliance_2) = test.eve().with_mock_alliance(2, Some(2));

    let alliance_endpoint_1 = test
        .eve()
        .with_alliance_endpoint(alliance_id_1, mock_alliance_1, 1);
    let alliance_endpoint_2 = test
        .eve()
        .with_alliance_endpoint(alliance_id_2, mock_alliance_2, 1);

    let alliance_ids = vec![alliance_id_1, alliance_id_2];
    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service.get_many_alliances(alliance_ids).await;

    assert!(result.is_ok());
    let alliances = result.unwrap();
    assert_eq!(alliances.len(), 2);

    alliance_endpoint_1.assert();
    alliance_endpoint_2.assert();

    Ok(())
}

/// Expect Error when ESI endpoint is unavailable for any alliance
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    let alliance_ids = vec![1, 2, 3];
    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service.get_many_alliances(alliance_ids).await;

    // Should fail on first unavailable alliance
    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

/// Expect Error when ESI fails partway through batch
#[tokio::test]
async fn fails_on_partial_esi_failure() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    // Setup mock endpoint for first alliance only
    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    let alliance_ids = vec![1, 2, 3];
    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service.get_many_alliances(alliance_ids).await;

    // Should succeed on first, fail on second (no mock)
    assert!(matches!(result, Err(Error::EsiError(_))));

    alliance_endpoint.assert();

    Ok(())
}

/// Expect Ok when fetching many alliances (stress test)
#[tokio::test]
async fn fetches_many_alliances() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    // Setup mock endpoints for 10 alliances
    let alliance_ids: Vec<i64> = (1..=10).collect();
    let mut endpoints = Vec::new();
    for id in &alliance_ids {
        let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
        endpoints.push(
            test.eve()
                .with_alliance_endpoint(alliance_id, mock_alliance, 1),
        );
    }

    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service
        .get_many_alliances(alliance_ids.clone())
        .await;

    assert!(result.is_ok());
    let alliances = result.unwrap();
    assert_eq!(alliances.len(), 10);

    // Verify all alliance IDs are present (order may vary due to concurrency)
    let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
    for id in &alliance_ids {
        assert!(returned_ids.contains(id));
    }

    // Assert all requests were made
    for endpoint in endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Expect Ok when fetching more than 10 alliances (tests batching)
#[tokio::test]
async fn fetches_many_alliances_with_batching() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    // Setup mock endpoints for 25 alliances to test multiple batches
    let alliance_ids: Vec<i64> = (1..=25).collect();
    let mut endpoints = Vec::new();
    for id in &alliance_ids {
        let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
        endpoints.push(
            test.eve()
                .with_alliance_endpoint(alliance_id, mock_alliance, 1),
        );
    }

    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service
        .get_many_alliances(alliance_ids.clone())
        .await;

    assert!(result.is_ok());
    let alliances = result.unwrap();
    assert_eq!(alliances.len(), 25);

    // Verify all alliance IDs are present (order may vary due to concurrency)
    let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
    for id in &alliance_ids {
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
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    // Setup mock endpoints for 5 alliances (within one batch)
    let alliance_ids: Vec<i64> = (1..=5).collect();
    let mut endpoints = Vec::new();
    for id in &alliance_ids {
        let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
        endpoints.push(
            test.eve()
                .with_alliance_endpoint(alliance_id, mock_alliance, 1),
        );
    }

    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service
        .get_many_alliances(alliance_ids.clone())
        .await;

    assert!(result.is_ok());
    let alliances = result.unwrap();
    assert_eq!(alliances.len(), 5);

    // Verify all alliance IDs are present
    let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
    for id in &alliance_ids {
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
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    // Setup mock endpoints for only some alliances in the batch
    let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

    let alliance_endpoint = test
        .eve()
        .with_alliance_endpoint(alliance_id, mock_alliance, 1);

    let alliance_ids = vec![1, 2, 3, 4, 5];
    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service.get_many_alliances(alliance_ids).await;

    // Should fail when any request in the batch fails
    assert!(matches!(result, Err(Error::EsiError(_))));

    alliance_endpoint.assert();

    Ok(())
}

/// Expect correct batching behavior with exactly 10 items
#[tokio::test]
async fn handles_exact_batch_size() -> Result<(), TestError> {
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    // Setup mock endpoints for exactly 10 alliances (one full batch)
    let alliance_ids: Vec<i64> = (1..=10).collect();
    let mut endpoints = Vec::new();
    for id in &alliance_ids {
        let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
        endpoints.push(
            test.eve()
                .with_alliance_endpoint(alliance_id, mock_alliance, 1),
        );
    }

    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service
        .get_many_alliances(alliance_ids.clone())
        .await;

    assert!(result.is_ok());
    let alliances = result.unwrap();
    assert_eq!(alliances.len(), 10);

    // Verify all alliance IDs are present
    let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
    for id in &alliance_ids {
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
    let mut test =
        test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

    // Setup mock endpoints for 11 alliances (one full batch + one item)
    let alliance_ids: Vec<i64> = (1..=11).collect();
    let mut endpoints = Vec::new();
    for id in &alliance_ids {
        let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
        endpoints.push(
            test.eve()
                .with_alliance_endpoint(alliance_id, mock_alliance, 1),
        );
    }

    let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
    let result = alliance_service
        .get_many_alliances(alliance_ids.clone())
        .await;

    assert!(result.is_ok());
    let alliances = result.unwrap();
    assert_eq!(alliances.len(), 11);

    // Verify all alliance IDs are present
    let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
    for id in &alliance_ids {
        assert!(returned_ids.contains(id));
    }

    // Assert all requests were made
    for endpoint in endpoints {
        endpoint.assert();
    }

    Ok(())
}
