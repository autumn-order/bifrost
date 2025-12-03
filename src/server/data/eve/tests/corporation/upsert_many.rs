//! Tests for CorporationRepository::upsert_many method.
//!
//! This module verifies the corporation upsert behavior, including inserting new
//! corporations, updating existing corporations, handling mixed batches, alliance
//! and faction relationships, and large batch operations.

use super::*;

/// Tests upserting new corporations.
///
/// Verifies that the corporation repository successfully inserts new corporation
/// records into the database.
///
/// Expected: Ok with Vec containing 2 created corporations
#[tokio::test]
async fn upserts_new_corporations() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let (corporation_id_1, corporation_1) = test.eve().mock_corporation(1, None, None);
    let (corporation_id_2, corporation_2) = test.eve().mock_corporation(2, None, None);

    let corporation_repo = CorporationRepository::new(&test.db);
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

/// Tests updating existing corporations.
///
/// Verifies that the corporation repository updates existing corporation records when
/// upserting with the same corporation IDs, preserving created_at and updating
/// info_updated_at and modified fields.
///
/// Expected: Ok with updated corporations, preserved created_at, newer info_updated_at
#[tokio::test]
async fn updates_existing_corporations() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let (corporation_id_1, corporation_1) = test.eve().mock_corporation(1, None, None);
    let (corporation_id_2, corporation_2) = test.eve().mock_corporation(2, None, None);
    let (corporation_id_1_update, mut corporation_1_update) =
        test.eve().mock_corporation(1, None, None);
    let (corporation_id_2_update, mut corporation_2_update) =
        test.eve().mock_corporation(2, None, None);

    let corporation_repo = CorporationRepository::new(&test.db);
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

    // Modify corporation data to verify updates
    corporation_1_update.name = "Updated Corporation 1".to_string();
    corporation_2_update.name = "Updated Corporation 2".to_string();

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
    assert_eq!(latest_entry_1.name, "Updated Corporation 1");
    assert_eq!(latest_entry_2.created_at, initial_created_at_2);
    assert!(latest_entry_2.info_updated_at > initial_updated_at_2);
    assert_eq!(latest_entry_2.name, "Updated Corporation 2");

    Ok(())
}

/// Tests upserting mixed new and existing corporations.
///
/// Verifies that the corporation repository correctly handles a batch containing
/// both new corporations (to insert) and existing corporations (to update) in a
/// single operation.
///
/// Expected: Ok with Vec containing both updated and newly created corporations
#[tokio::test]
async fn upserts_mixed_new_and_existing_corporations() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let (corporation_id_1, corporation_1) = test.eve().mock_corporation(1, None, None);
    let (corporation_id_2, corporation_2) = test.eve().mock_corporation(2, None, None);
    let (corporation_id_3, corporation_3) = test.eve().mock_corporation(3, None, None);
    let (corporation_id_1_update, mut corporation_1_update) =
        test.eve().mock_corporation(1, None, None);
    corporation_1_update.name = "Updated Corporation 1".to_string();
    let (corporation_id_2_update, corporation_2_update) =
        test.eve().mock_corporation(2, None, None);

    let corporation_repo = CorporationRepository::new(&test.db);

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

/// Tests handling empty input.
///
/// Verifies that the corporation repository handles empty upsert lists gracefully
/// without errors.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn handles_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo.upsert_many(vec![]).await?;

    assert_eq!(result.len(), 0);

    Ok(())
}

/// Tests upserting corporations with alliance and faction relationships.
///
/// Verifies that the corporation repository correctly handles corporations with
/// various alliance and faction relationships (Some alliance, different alliances,
/// None, and faction affiliations).
///
/// Expected: Ok with corporations having correct alliance_id and faction_id assignments
#[tokio::test]
async fn upserts_with_alliance_and_faction_relationships() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let alliance_1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance_2 = test.eve().insert_mock_alliance(2, None).await?;
    let faction_1 = test.eve().insert_mock_faction(1).await?;
    let faction_2 = test.eve().insert_mock_faction(2).await?;

    let (corporation_id_1, corporation_1) = test.eve().mock_corporation(1, None, None);
    let (corporation_id_2, corporation_2) = test.eve().mock_corporation(2, None, None);
    let (corporation_id_3, corporation_3) = test.eve().mock_corporation(3, None, None);
    let (corporation_id_4, corporation_4) = test.eve().mock_corporation(4, None, None);

    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo
        .upsert_many(vec![
            (
                corporation_id_1,
                corporation_1,
                Some(alliance_1.id),
                Some(faction_1.id),
            ),
            (
                corporation_id_2,
                corporation_2,
                Some(alliance_2.id),
                Some(faction_2.id),
            ),
            (corporation_id_3, corporation_3, Some(alliance_1.id), None),
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
    assert_eq!(corp_2.faction_id, Some(faction_2.id));
    assert_eq!(corp_3.alliance_id, Some(alliance_1.id));
    assert_eq!(corp_3.faction_id, None);
    assert_eq!(corp_4.alliance_id, None);
    assert_eq!(corp_4.faction_id, None);

    Ok(())
}

/// Tests handling large batch upsert.
///
/// Verifies that the corporation repository efficiently handles upserting large
/// batches of corporations (100 in this test) in a single operation.
///
/// Expected: Ok with Vec containing all 100 corporations
#[tokio::test]
async fn handles_large_batch() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    let mut corporations = Vec::new();
    for i in 1..=100 {
        let (corporation_id, corporation) = test.eve().mock_corporation(i, None, None);
        corporations.push((corporation_id, corporation, None, None));
    }

    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo.upsert_many(corporations).await?;

    assert_eq!(result.len(), 100);

    // Verify all corporation IDs are present
    for i in 1..=100 {
        assert!(result.iter().any(|c| c.corporation_id == i));
    }

    Ok(())
}
