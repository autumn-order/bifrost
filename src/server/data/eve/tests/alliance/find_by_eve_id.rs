//! Tests for AllianceRepository::find_by_eve_id method.
//!
//! This module verifies the alliance lookup behavior by EVE alliance ID,
//! including finding existing alliances and handling non-existent alliances.

use super::*;

/// Tests finding an existing alliance by EVE ID.
///
/// Verifies that the alliance repository successfully finds and returns an
/// alliance record when searching by a valid EVE alliance ID.
///
/// Expected: Ok(Some(alliance)) with matching alliance data
#[tokio::test]
async fn finds_existing_alliance() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let (alliance_id, alliance) = test.eve().mock_alliance(1, None);

    let alliance_repo = AllianceRepository::new(&test.db);
    alliance_repo
        .upsert_many(vec![(alliance_id, alliance.clone(), None)])
        .await?;

    let result = alliance_repo.find_by_eve_id(alliance_id).await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let found_alliance = result.unwrap();
    assert!(found_alliance.is_some());

    let found = found_alliance.unwrap();
    assert_eq!(found.alliance_id, alliance_id);
    assert_eq!(found.name, alliance.name);
    assert_eq!(found.ticker, alliance.ticker);
    assert_eq!(found.creator_id, alliance.creator_id);

    Ok(())
}

/// Tests finding a non-existent alliance by EVE ID.
///
/// Verifies that the alliance repository returns None when searching for
/// an alliance ID that doesn't exist in the database.
///
/// Expected: Ok(None)
#[tokio::test]
async fn returns_none_for_nonexistent_alliance() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;

    let alliance_repo = AllianceRepository::new(&test.db);
    let result = alliance_repo.find_by_eve_id(999999999).await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let found_alliance = result.unwrap();
    assert!(found_alliance.is_none());

    Ok(())
}

/// Tests finding alliances with faction affiliations.
///
/// Verifies that the alliance repository correctly retrieves alliances
/// with their faction relationships intact.
///
/// Expected: Ok(Some(alliance)) with correct faction_id
#[tokio::test]
async fn finds_alliance_with_faction() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let faction = test.eve().insert_mock_faction(1).await?;
    let (alliance_id, alliance) = test.eve().mock_alliance(1, Some(1));

    let alliance_repo = AllianceRepository::new(&test.db);
    alliance_repo
        .upsert_many(vec![(alliance_id, alliance, Some(faction.id))])
        .await?;

    let result = alliance_repo.find_by_eve_id(alliance_id).await?;

    assert!(result.is_some());
    let found = result.unwrap();
    assert_eq!(found.alliance_id, alliance_id);
    assert_eq!(found.faction_id, Some(faction.id));

    Ok(())
}

/// Tests finding multiple different alliances.
///
/// Verifies that the alliance repository can correctly find different
/// alliances when multiple alliances exist in the database.
///
/// Expected: Ok(Some(alliance)) for each searched alliance ID
#[tokio::test]
async fn finds_correct_alliance_among_multiple() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let (alliance_id_1, alliance_1) = test.eve().mock_alliance(1, None);
    let (alliance_id_2, alliance_2) = test.eve().mock_alliance(2, None);
    let (alliance_id_3, alliance_3) = test.eve().mock_alliance(3, None);

    let alliance_repo = AllianceRepository::new(&test.db);
    alliance_repo
        .upsert_many(vec![
            (alliance_id_1, alliance_1.clone(), None),
            (alliance_id_2, alliance_2.clone(), None),
            (alliance_id_3, alliance_3.clone(), None),
        ])
        .await?;

    // Find each alliance and verify correct data is returned
    let found_1 = alliance_repo.find_by_eve_id(alliance_id_1).await?.unwrap();
    let found_2 = alliance_repo.find_by_eve_id(alliance_id_2).await?.unwrap();
    let found_3 = alliance_repo.find_by_eve_id(alliance_id_3).await?.unwrap();

    assert_eq!(found_1.alliance_id, alliance_id_1);
    assert_eq!(found_1.name, alliance_1.name);
    assert_eq!(found_2.alliance_id, alliance_id_2);
    assert_eq!(found_2.name, alliance_2.name);
    assert_eq!(found_3.alliance_id, alliance_id_3);
    assert_eq!(found_3.name, alliance_3.name);

    Ok(())
}
