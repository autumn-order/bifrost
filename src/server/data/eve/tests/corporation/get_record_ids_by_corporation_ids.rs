use super::*;

/// Expect Ok with correct mappings when corporations exist in database
#[tokio::test]
async fn returns_record_ids_for_existing_corporations() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let corporation_1 = test.eve().insert_mock_corporation(1, None, None).await?;
    let corporation_2 = test.eve().insert_mock_corporation(2, None, None).await?;
    let corporation_3 = test.eve().insert_mock_corporation(3, None, None).await?;

    let corporation_repo = CorporationRepository::new(&test.state.db);
    let corporation_ids = vec![
        corporation_1.corporation_id,
        corporation_2.corporation_id,
        corporation_3.corporation_id,
    ];
    let result = corporation_repo
        .get_record_ids_by_corporation_ids(&corporation_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 3);

    // Verify the mappings are correct
    let mut found_ids = std::collections::HashSet::new();
    for (record_id, corporation_id) in record_ids {
        match corporation_id {
            _ if corporation_id == corporation_1.corporation_id => {
                assert_eq!(record_id, corporation_1.id);
            }
            _ if corporation_id == corporation_2.corporation_id => {
                assert_eq!(record_id, corporation_2.id);
            }
            _ if corporation_id == corporation_3.corporation_id => {
                assert_eq!(record_id, corporation_3.id);
            }
            _ => panic!("Unexpected corporation_id: {}", corporation_id),
        }
        found_ids.insert(corporation_id);
    }
    assert_eq!(found_ids.len(), 3);

    Ok(())
}

/// Expect Ok with empty Vec when no corporations match
#[tokio::test]
async fn returns_empty_for_nonexistent_corporations() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    let corporation_repo = CorporationRepository::new(&test.state.db);
    let corporation_ids = vec![1, 2, 3];
    let result = corporation_repo
        .get_record_ids_by_corporation_ids(&corporation_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Expect Ok with empty Vec when input is empty
#[tokio::test]
async fn returns_empty_for_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    let corporation_repo = CorporationRepository::new(&test.state.db);
    let corporation_ids: Vec<i64> = vec![];
    let result = corporation_repo
        .get_record_ids_by_corporation_ids(&corporation_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 0);

    Ok(())
}

/// Expect Ok with partial results when only some corporations exist
#[tokio::test]
async fn returns_partial_results_for_mixed_input() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let corporation_1 = test.eve().insert_mock_corporation(1, None, None).await?;
    let corporation_3 = test.eve().insert_mock_corporation(3, None, None).await?;

    let corporation_repo = CorporationRepository::new(&test.state.db);
    let corporation_ids = vec![
        corporation_1.corporation_id,
        999, // Non-existent
        corporation_3.corporation_id,
        888, // Non-existent
    ];
    let result = corporation_repo
        .get_record_ids_by_corporation_ids(&corporation_ids)
        .await;

    assert!(result.is_ok());
    let record_ids = result.unwrap();
    assert_eq!(record_ids.len(), 2);

    // Verify only existing corporations are returned
    for (record_id, corporation_id) in record_ids {
        assert!(
            corporation_id == corporation_1.corporation_id
                || corporation_id == corporation_3.corporation_id
        );
        if corporation_id == corporation_1.corporation_id {
            assert_eq!(record_id, corporation_1.id);
        } else if corporation_id == corporation_3.corporation_id {
            assert_eq!(record_id, corporation_3.id);
        }
    }

    Ok(())
}

/// Expect Error when required tables haven't been created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = TestBuilder::new().build().await?;

    let corporation_repo = CorporationRepository::new(&test.state.db);
    let corporation_ids = vec![1, 2, 3];
    let result = corporation_repo
        .get_record_ids_by_corporation_ids(&corporation_ids)
        .await;

    assert!(result.is_err());

    Ok(())
}
