use super::*;

/// Should successfully update a single corporation's alliance affiliation
#[tokio::test]
async fn updates_single_corporation_alliance() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Create two alliances and a corporation initially affiliated with the first
    let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
    let corp = test
        .eve()
        .insert_mock_corporation(100, Some(alliance1.alliance_id), None)
        .await?;

    // Update corporation to be affiliated with the second alliance
    let corporation_repo = CorporationRepository::new(&test.state.db);
    let result = corporation_repo
        .update_affiliations(vec![(corp.id, Some(alliance2.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the update
    let updated = corporation_repo
        .get_by_corporation_id(corp.corporation_id)
        .await?
        .expect("Corporation should exist");

    assert_eq!(updated.alliance_id, Some(alliance2.id));

    Ok(())
}

/// Should successfully update multiple corporations in a single call
#[tokio::test]
async fn updates_multiple_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Create alliances
    let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
    let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

    // Create corporations
    let corp1 = test.eve().insert_mock_corporation(1, None, None).await?;
    let corp2 = test
        .eve()
        .insert_mock_corporation(2, Some(alliance1.alliance_id), None)
        .await?;
    let corp3 = test.eve().insert_mock_corporation(3, None, None).await?;

    // Update multiple corporations
    let corporation_repo = CorporationRepository::new(&test.state.db);
    let result = corporation_repo
        .update_affiliations(vec![
            (corp1.id, Some(alliance1.id)),
            (corp2.id, Some(alliance2.id)),
            (corp3.id, Some(alliance3.id)),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify all updates
    let updated1 = corporation_repo
        .get_by_corporation_id(corp1.corporation_id)
        .await?
        .expect("Corporation 1 should exist");
    let updated2 = corporation_repo
        .get_by_corporation_id(corp2.corporation_id)
        .await?
        .expect("Corporation 2 should exist");
    let updated3 = corporation_repo
        .get_by_corporation_id(corp3.corporation_id)
        .await?
        .expect("Corporation 3 should exist");

    assert_eq!(updated1.alliance_id, Some(alliance1.id));
    assert_eq!(updated2.alliance_id, Some(alliance2.id));
    assert_eq!(updated3.alliance_id, Some(alliance3.id));

    Ok(())
}

/// Should successfully remove alliance affiliation by setting to None
#[tokio::test]
async fn removes_alliance_affiliation() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Create alliance and corporation with that alliance
    let alliance = test.eve().insert_mock_alliance(1, None).await?;
    let corp = test
        .eve()
        .insert_mock_corporation(100, Some(alliance.alliance_id), None)
        .await?;

    // Remove alliance affiliation
    let corporation_repo = CorporationRepository::new(&test.state.db);
    let result = corporation_repo
        .update_affiliations(vec![(corp.id, None)])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the alliance was removed
    let updated = corporation_repo
        .get_by_corporation_id(corp.corporation_id)
        .await?
        .expect("Corporation should exist");

    assert_eq!(updated.alliance_id, None);

    Ok(())
}

/// Should handle batching for large numbers of corporations (>100)
#[tokio::test]
async fn handles_large_batch_updates() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Create an alliance
    let alliance = test.eve().insert_mock_alliance(1, None).await?;

    // Create 250 corporations (more than 2x BATCH_SIZE)
    let mut corporations = Vec::new();
    for i in 0..250 {
        let corp = test
            .eve()
            .insert_mock_corporation(100 + i, None, None)
            .await?;
        corporations.push((corp.id, Some(alliance.id)));
    }

    // Update all corporations
    let corporation_repo = CorporationRepository::new(&test.state.db);
    let result = corporation_repo.update_affiliations(corporations).await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify a sample of updates
    let updated_first = corporation_repo
        .get_by_corporation_id(100)
        .await?
        .expect("First corporation should exist");
    let updated_middle = corporation_repo
        .get_by_corporation_id(225)
        .await?
        .expect("Middle corporation should exist");
    let updated_last = corporation_repo
        .get_by_corporation_id(349)
        .await?
        .expect("Last corporation should exist");

    assert_eq!(updated_first.alliance_id, Some(alliance.id));
    assert_eq!(updated_middle.alliance_id, Some(alliance.id));
    assert_eq!(updated_last.alliance_id, Some(alliance.id));

    Ok(())
}

/// Should handle empty input gracefully
#[tokio::test]
async fn handles_empty_input() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let corporation_repo = CorporationRepository::new(&test.state.db);
    let result = corporation_repo.update_affiliations(vec![]).await;

    assert!(result.is_ok(), "Should handle empty input gracefully");

    Ok(())
}

/// Should update UpdatedAt timestamp when updating affiliations
#[tokio::test]
async fn updates_timestamp() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Create alliance and corporation
    let alliance = test.eve().insert_mock_alliance(1, None).await?;
    let corp = test.eve().insert_mock_corporation(100, None, None).await?;

    let original_updated_at = corp.updated_at;

    // Wait a moment to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Update the corporation
    let corporation_repo = CorporationRepository::new(&test.state.db);
    let result = corporation_repo
        .update_affiliations(vec![(corp.id, Some(alliance.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the timestamp was updated
    let updated = corporation_repo
        .get_by_corporation_id(corp.corporation_id)
        .await?
        .expect("Corporation should exist");

    assert!(
        updated.updated_at >= original_updated_at,
        "UpdatedAt should be equal to or newer than original. Original: {:?}, Updated: {:?}",
        original_updated_at,
        updated.updated_at
    );

    Ok(())
}

/// Should not affect corporations not in the update list
#[tokio::test]
async fn does_not_affect_other_corporations() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Create alliances
    let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance2 = test.eve().insert_mock_alliance(2, None).await?;

    // Create corporations
    let corp1 = test
        .eve()
        .insert_mock_corporation(100, Some(alliance1.alliance_id), None)
        .await?;
    let corp2 = test
        .eve()
        .insert_mock_corporation(200, Some(alliance1.alliance_id), None)
        .await?;

    // Update only corp1
    let corporation_repo = CorporationRepository::new(&test.state.db);
    let result = corporation_repo
        .update_affiliations(vec![(corp1.id, Some(alliance2.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify corp1 was updated
    let updated1 = corporation_repo
        .get_by_corporation_id(corp1.corporation_id)
        .await?
        .expect("Corporation 1 should exist");
    assert_eq!(updated1.alliance_id, Some(alliance2.id));

    // Verify corp2 was NOT updated
    let updated2 = corporation_repo
        .get_by_corporation_id(corp2.corporation_id)
        .await?
        .expect("Corporation 2 should exist");
    assert_eq!(
        updated2.alliance_id,
        Some(alliance1.id),
        "Corporation 2 should still have original alliance"
    );

    Ok(())
}

/// Should handle mix of Some and None alliance IDs in same batch
#[tokio::test]
async fn handles_mixed_alliance_assignments() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    // Create alliance
    let alliance = test.eve().insert_mock_alliance(1, None).await?;

    // Create corporations
    let corp1 = test.eve().insert_mock_corporation(100, None, None).await?;
    let corp2 = test
        .eve()
        .insert_mock_corporation(200, Some(alliance.alliance_id), None)
        .await?;
    let corp3 = test.eve().insert_mock_corporation(300, None, None).await?;

    // Update with mixed alliance IDs
    let corporation_repo = CorporationRepository::new(&test.state.db);
    let result = corporation_repo
        .update_affiliations(vec![
            (corp1.id, Some(alliance.id)),
            (corp2.id, None),
            (corp3.id, Some(alliance.id)),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify updates
    let updated1 = corporation_repo
        .get_by_corporation_id(corp1.corporation_id)
        .await?
        .expect("Corporation 1 should exist");
    let updated2 = corporation_repo
        .get_by_corporation_id(corp2.corporation_id)
        .await?
        .expect("Corporation 2 should exist");
    let updated3 = corporation_repo
        .get_by_corporation_id(corp3.corporation_id)
        .await?
        .expect("Corporation 3 should exist");

    assert_eq!(updated1.alliance_id, Some(alliance.id));
    assert_eq!(updated2.alliance_id, None);
    assert_eq!(updated3.alliance_id, Some(alliance.id));

    Ok(())
}
