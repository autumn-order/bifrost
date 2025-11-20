use super::*;

/// Expect Ok when upserting new corporations
#[tokio::test]
async fn upserts_new_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let (corporation_id_1, corporation_1) = test.eve().with_mock_corporation(1, None, None);
    let (corporation_id_2, corporation_2) = test.eve().with_mock_corporation(2, None, None);

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert_many(vec![
            (corporation_id_1, corporation_1, None, None),
            (corporation_id_2, corporation_2, None, None),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let created_corporations = result.unwrap();
    assert_eq!(created_corporations.len(), 2);

    Ok(())
}

/// Expect Ok & update when trying to upsert existing corporations
#[tokio::test]
async fn updates_existing_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let (corporation_id_1, corporation_1) = test.eve().with_mock_corporation(1, None, None);
    let (corporation_id_2, corporation_2) = test.eve().with_mock_corporation(2, None, None);
    let (corporation_id_1_update, corporation_1_update) =
        test.eve().with_mock_corporation(1, None, None);
    let (corporation_id_2_update, corporation_2_update) =
        test.eve().with_mock_corporation(2, None, None);

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let initial = corporation_repo
        .upsert_many(vec![
            (corporation_id_1, corporation_1, None, None),
            (corporation_id_2, corporation_2, None, None),
        ])
        .await?;

    let initial_entry_1 = initial
        .iter()
        .find(|c| c.corporation_id == corporation_id_1)
        .expect("corporation 1 not found");
    let initial_entry_2 = initial
        .iter()
        .find(|c| c.corporation_id == corporation_id_2)
        .expect("corporation 2 not found");

    let initial_created_at_1 = initial_entry_1.created_at;
    let initial_updated_at_1 = initial_entry_1.info_updated_at;
    let initial_created_at_2 = initial_entry_2.created_at;
    let initial_updated_at_2 = initial_entry_2.info_updated_at;

    let latest = corporation_repo
        .upsert_many(vec![
            (corporation_id_1_update, corporation_1_update, None, None),
            (corporation_id_2_update, corporation_2_update, None, None),
        ])
        .await?;

    let latest_entry_1 = latest
        .iter()
        .find(|c| c.corporation_id == corporation_id_1_update)
        .expect("corporation 1 not found");
    let latest_entry_2 = latest
        .iter()
        .find(|c| c.corporation_id == corporation_id_2_update)
        .expect("corporation 2 not found");

    // created_at should not change and updated_at should increase for both corporations
    assert_eq!(latest_entry_1.created_at, initial_created_at_1);
    assert!(latest_entry_1.info_updated_at > initial_updated_at_1);
    assert_eq!(latest_entry_2.created_at, initial_created_at_2);
    assert!(latest_entry_2.info_updated_at > initial_updated_at_2);

    Ok(())
}

/// Expect Ok when upserting mix of new and existing corporations
#[tokio::test]
async fn upserts_mixed_new_and_existing_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let (corporation_id_1, corporation_1) = test.eve().with_mock_corporation(1, None, None);
    let (corporation_id_2, corporation_2) = test.eve().with_mock_corporation(2, None, None);
    let (corporation_id_3, corporation_3) = test.eve().with_mock_corporation(3, None, None);
    let (corporation_id_1_update, mut corporation_1_update) =
        test.eve().with_mock_corporation(1, None, None);
    corporation_1_update.name = "Updated Corporation 1".to_string();
    let (corporation_id_2_update, corporation_2_update) =
        test.eve().with_mock_corporation(2, None, None);

    let corporation_repo = CorporationRepository::new(test.state.db.clone());

    // First, insert corporations 1 and 2
    let initial = corporation_repo
        .upsert_many(vec![
            (corporation_id_1, corporation_1, None, None),
            (corporation_id_2, corporation_2, None, None),
        ])
        .await?;

    assert_eq!(initial.len(), 2);
    let initial_corp_1 = initial
        .iter()
        .find(|c| c.corporation_id == corporation_id_1)
        .expect("corporation 1 not found");
    let initial_created_at = initial_corp_1.created_at;

    // Now upsert corporations 1 (update), 2 (update), and 3 (new)
    let result = corporation_repo
        .upsert_many(vec![
            (corporation_id_1_update, corporation_1_update, None, None),
            (corporation_id_2_update, corporation_2_update, None, None),
            (corporation_id_3, corporation_3, None, None),
        ])
        .await?;

    assert_eq!(result.len(), 3);

    let updated_corp_1 = result
        .iter()
        .find(|c| c.corporation_id == corporation_id_1)
        .expect("corporation 1 not found");
    let corp_3 = result
        .iter()
        .find(|c| c.corporation_id == corporation_id_3)
        .expect("corporation 3 not found");

    // Corporation 1 should be updated (same created_at, changed name)
    assert_eq!(updated_corp_1.created_at, initial_created_at);
    assert_eq!(updated_corp_1.name, "Updated Corporation 1");

    // Corporation 3 should be newly created
    assert_eq!(corp_3.corporation_id, corporation_id_3);

    Ok(())
}

/// Expect Ok with empty result when upserting empty vector
#[tokio::test]
async fn handles_empty_input() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo.upsert_many(vec![]).await?;

    assert_eq!(result.len(), 0);

    Ok(())
}

/// Expect Ok when upserting corporations with various alliance and faction relationships
#[tokio::test]
async fn upserts_with_alliance_and_faction_relationships() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let alliance_1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance_2 = test.eve().insert_mock_alliance(2, None).await?;
    let faction_1 = test.eve().insert_mock_faction(1).await?;
    let faction_2 = test.eve().insert_mock_faction(2).await?;

    let (corporation_id_1, corporation_1) = test.eve().with_mock_corporation(
        1,
        Some(alliance_1.alliance_id),
        Some(faction_1.faction_id),
    );
    let (corporation_id_2, corporation_2) =
        test.eve()
            .with_mock_corporation(2, Some(alliance_2.alliance_id), None);
    let (corporation_id_3, corporation_3) =
        test.eve()
            .with_mock_corporation(3, None, Some(faction_2.faction_id));
    let (corporation_id_4, corporation_4) = test.eve().with_mock_corporation(4, None, None);

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert_many(vec![
            (
                corporation_id_1,
                corporation_1,
                Some(alliance_1.id),
                Some(faction_1.id),
            ),
            (corporation_id_2, corporation_2, Some(alliance_2.id), None),
            (corporation_id_3, corporation_3, None, Some(faction_2.id)),
            (corporation_id_4, corporation_4, None, None),
        ])
        .await?;

    assert_eq!(result.len(), 4);

    let corp_1 = result
        .iter()
        .find(|c| c.corporation_id == corporation_id_1)
        .unwrap();
    let corp_2 = result
        .iter()
        .find(|c| c.corporation_id == corporation_id_2)
        .unwrap();
    let corp_3 = result
        .iter()
        .find(|c| c.corporation_id == corporation_id_3)
        .unwrap();
    let corp_4 = result
        .iter()
        .find(|c| c.corporation_id == corporation_id_4)
        .unwrap();

    assert_eq!(corp_1.alliance_id, Some(alliance_1.id));
    assert_eq!(corp_1.faction_id, Some(faction_1.id));
    assert_eq!(corp_2.alliance_id, Some(alliance_2.id));
    assert_eq!(corp_2.faction_id, None);
    assert_eq!(corp_3.alliance_id, None);
    assert_eq!(corp_3.faction_id, Some(faction_2.id));
    assert_eq!(corp_4.alliance_id, None);
    assert_eq!(corp_4.faction_id, None);

    Ok(())
}

/// Expect Ok when upserting large batch of corporations
#[tokio::test]
async fn handles_large_batch() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let mut corporations = Vec::new();
    for i in 1..=100 {
        let (corporation_id, corporation) = test.eve().with_mock_corporation(i, None, None);
        corporations.push((corporation_id, corporation, None, None));
    }

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo.upsert_many(corporations).await?;

    assert_eq!(result.len(), 100);

    // Verify all corporation IDs are present
    for i in 1..=100 {
        assert!(result.iter().any(|c| c.corporation_id == i));
    }

    Ok(())
}
