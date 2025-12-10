//! Tests for CorporationRepository::find_by_eve_id method.
//!
//! This module verifies the corporation lookup behavior by EVE corporation ID,
//! including finding existing corporations and handling non-existent corporations.

use super::*;

/// Tests finding an existing corporation by EVE ID.
///
/// Verifies that the corporation repository successfully finds and returns a
/// corporation record when searching by a valid EVE corporation ID.
///
/// Expected: Ok(Some(corporation)) with matching corporation data
#[tokio::test]
async fn finds_existing_corporation() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let (corporation_id, corporation) = test.eve().mock_corporation(1, None, None);

    let corporation_repo = CorporationRepository::new(&test.db);
    corporation_repo
        .upsert_many(vec![(corporation_id, corporation.clone(), None, None)])
        .await?;

    let result = corporation_repo.find_by_eve_id(corporation_id).await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let found_corporation = result.unwrap();
    assert!(found_corporation.is_some());

    let found = found_corporation.unwrap();
    assert_eq!(found.corporation_id, corporation_id);
    assert_eq!(found.name, corporation.name);
    assert_eq!(found.ticker, corporation.ticker);
    assert_eq!(found.ceo_id, corporation.ceo_id);
    assert_eq!(found.member_count, corporation.member_count);

    Ok(())
}

/// Tests finding a non-existent corporation by EVE ID.
///
/// Verifies that the corporation repository returns None when searching for
/// a corporation ID that doesn't exist in the database.
///
/// Expected: Ok(None)
#[tokio::test]
async fn returns_none_for_nonexistent_corporation() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo.find_by_eve_id(999999999).await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let found_corporation = result.unwrap();
    assert!(found_corporation.is_none());

    Ok(())
}

/// Tests finding corporations with alliance and faction affiliations.
///
/// Verifies that the corporation repository correctly retrieves corporations
/// with their alliance and faction relationships intact.
///
/// Expected: Ok(Some(corporation)) with correct alliance_id and faction_id
#[tokio::test]
async fn finds_corporation_with_affiliations() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let alliance = test.eve().insert_mock_alliance(1, None).await?;
    let faction = test.eve().insert_mock_faction(1).await?;
    let (corporation_id, corporation) = test.eve().mock_corporation(1, None, None);

    let corporation_repo = CorporationRepository::new(&test.db);
    corporation_repo
        .upsert_many(vec![(
            corporation_id,
            corporation,
            Some(alliance.id),
            Some(faction.id),
        )])
        .await?;

    let result = corporation_repo.find_by_eve_id(corporation_id).await?;

    assert!(result.is_some());
    let found = result.unwrap();
    assert_eq!(found.corporation_id, corporation_id);
    assert_eq!(found.alliance_id, Some(alliance.id));
    assert_eq!(found.faction_id, Some(faction.id));

    Ok(())
}

/// Tests finding multiple different corporations.
///
/// Verifies that the corporation repository can correctly find different
/// corporations when multiple corporations exist in the database.
///
/// Expected: Ok(Some(corporation)) for each searched corporation ID
#[tokio::test]
async fn finds_correct_corporation_among_multiple() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let (corporation_id_1, corporation_1) = test.eve().mock_corporation(1, None, None);
    let (corporation_id_2, corporation_2) = test.eve().mock_corporation(2, None, None);
    let (corporation_id_3, corporation_3) = test.eve().mock_corporation(3, None, None);

    let corporation_repo = CorporationRepository::new(&test.db);
    corporation_repo
        .upsert_many(vec![
            (corporation_id_1, corporation_1.clone(), None, None),
            (corporation_id_2, corporation_2.clone(), None, None),
            (corporation_id_3, corporation_3.clone(), None, None),
        ])
        .await?;

    // Find each corporation and verify correct data is returned
    let found_1 = corporation_repo
        .find_by_eve_id(corporation_id_1)
        .await?
        .unwrap();
    let found_2 = corporation_repo
        .find_by_eve_id(corporation_id_2)
        .await?
        .unwrap();
    let found_3 = corporation_repo
        .find_by_eve_id(corporation_id_3)
        .await?
        .unwrap();

    assert_eq!(found_1.corporation_id, corporation_id_1);
    assert_eq!(found_1.name, corporation_1.name);
    assert_eq!(found_2.corporation_id, corporation_id_2);
    assert_eq!(found_2.name, corporation_2.name);
    assert_eq!(found_3.corporation_id, corporation_id_3);
    assert_eq!(found_3.name, corporation_3.name);

    Ok(())
}
