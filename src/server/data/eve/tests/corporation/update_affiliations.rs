//! Tests for CorporationRepository::update_affiliations method.
//!
//! This module verifies the corporation affiliation update behavior, including updating
//! alliance affiliations for single and multiple corporations, handling batch operations,
//! timestamp updates, and edge cases like empty inputs and mixed alliance assignments.

use super::*;
use sea_orm::EntityTrait;

/// Tests updating a single corporation's alliance affiliation.
///
/// Verifies that the corporation repository successfully updates a corporation's
/// alliance affiliation in the database.
///
/// Expected: Ok with updated alliance_id
#[tokio::test]
async fn updates_single_corporation_alliance() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    // Create two alliances and a corporation initially affiliated with the first
    let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
    let corp = test
        .eve()
        .insert_mock_corporation(100, Some(alliance1.alliance_id), None)
        .await?;

    // Update corporation to be affiliated with the second alliance
    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo
        .update_affiliations(vec![(corp.id, Some(alliance2.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the update by querying directly
    let updated = entity::prelude::EveCorporation::find_by_id(corp.id)
        .one(&test.db)
        .await?
        .expect("Corporation should exist");

    assert_eq!(updated.alliance_id, Some(alliance2.id));

    Ok(())
}

/// Tests updating multiple corporations in a single call.
///
/// Verifies that the corporation repository successfully updates alliance affiliations
/// for multiple corporations in a single batch operation.
///
/// Expected: Ok with all corporations updated to their respective alliances
#[tokio::test]
async fn updates_multiple_corporations() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

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
    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo
        .update_affiliations(vec![
            (corp1.id, Some(alliance1.id)),
            (corp2.id, Some(alliance2.id)),
            (corp3.id, Some(alliance3.id)),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify all updates by querying directly
    let updated1 = entity::prelude::EveCorporation::find_by_id(corp1.id)
        .one(&test.db)
        .await?
        .expect("Corporation 1 should exist");
    let updated2 = entity::prelude::EveCorporation::find_by_id(corp2.id)
        .one(&test.db)
        .await?
        .expect("Corporation 2 should exist");
    let updated3 = entity::prelude::EveCorporation::find_by_id(corp3.id)
        .one(&test.db)
        .await?
        .expect("Corporation 3 should exist");

    assert_eq!(updated1.alliance_id, Some(alliance1.id));
    assert_eq!(updated2.alliance_id, Some(alliance2.id));
    assert_eq!(updated3.alliance_id, Some(alliance3.id));

    Ok(())
}

/// Tests removing alliance affiliation.
///
/// Verifies that the corporation repository successfully removes a corporation's
/// alliance affiliation by setting it to None.
///
/// Expected: Ok with alliance_id set to None
#[tokio::test]
async fn removes_alliance_affiliation() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    // Create alliance and corporation
    let alliance = test.eve().insert_mock_alliance(1, None).await?;
    let corp = test
        .eve()
        .insert_mock_corporation(100, Some(alliance.alliance_id), None)
        .await?;

    // Remove alliance affiliation
    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo
        .update_affiliations(vec![(corp.id, None)])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the alliance was removed
    let updated = entity::prelude::EveCorporation::find_by_id(corp.id)
        .one(&test.db)
        .await?
        .expect("Corporation should exist");

    assert_eq!(updated.alliance_id, None);

    Ok(())
}

/// Tests handling large batch updates.
///
/// Verifies that the corporation repository correctly handles batching when updating
/// affiliations for large numbers of corporations (>100), ensuring all updates are
/// processed across multiple batches.
///
/// Expected: Ok with all 250 corporations updated
#[tokio::test]
async fn handles_large_batch_updates() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    // Create an alliance
    let alliance = test.eve().insert_mock_alliance(1, None).await?;

    // Create 250 corporations (more than 2x BATCH_SIZE)
    let mut corporations = Vec::new();
    for i in 0..250 {
        let corp = test
            .eve()
            .insert_mock_corporation(1000 + i, None, None)
            .await?;

        corporations.push((corp.id, Some(alliance.id)));
    }

    // Update all corporations
    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo.update_affiliations(corporations).await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify a sample of updates using direct entity queries
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let updated_first = entity::prelude::EveCorporation::find()
        .filter(entity::eve_corporation::Column::CorporationId.eq(1000))
        .one(&test.db)
        .await?
        .expect("First corporation should exist");
    let updated_middle = entity::prelude::EveCorporation::find()
        .filter(entity::eve_corporation::Column::CorporationId.eq(1125))
        .one(&test.db)
        .await?
        .expect("Middle corporation should exist");
    let updated_last = entity::prelude::EveCorporation::find()
        .filter(entity::eve_corporation::Column::CorporationId.eq(1249))
        .one(&test.db)
        .await?
        .expect("Last corporation should exist");

    assert_eq!(updated_first.alliance_id, Some(alliance.id));
    assert_eq!(updated_middle.alliance_id, Some(alliance.id));
    assert_eq!(updated_last.alliance_id, Some(alliance.id));

    Ok(())
}

/// Tests handling empty input.
///
/// Verifies that the corporation repository handles empty affiliation update lists
/// gracefully without errors.
///
/// Expected: Ok with no operations performed
#[tokio::test]
async fn handles_empty_input() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo.update_affiliations(vec![]).await;

    assert!(result.is_ok(), "Should handle empty input gracefully");

    Ok(())
}

/// Tests updating affiliation timestamp.
///
/// Verifies that the corporation repository updates the affiliation_updated_at
/// timestamp whenever affiliation data is modified.
///
/// Expected: Ok with affiliation_updated_at newer than original
#[tokio::test]
async fn updates_timestamp() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    // Create alliance and corporation
    let alliance = test.eve().insert_mock_alliance(1, None).await?;
    let corp = test.eve().insert_mock_corporation(100, None, None).await?;

    let original_updated_at = corp.affiliation_updated_at;

    // Wait a moment to ensure timestamp difference (Utc::now() has nanosecond precision)
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Update the corporation
    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo
        .update_affiliations(vec![(corp.id, Some(alliance.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify the timestamp was updated
    let updated = entity::prelude::EveCorporation::find_by_id(corp.id)
        .one(&test.db)
        .await?
        .expect("Corporation should exist");

    assert!(
        updated.affiliation_updated_at > original_updated_at,
        "affiliation_updated_at should be newer than original. Original: {:?}, Updated: {:?}",
        original_updated_at,
        updated.affiliation_updated_at
    );

    Ok(())
}

/// Tests that other corporations are not affected.
///
/// Verifies that the corporation repository only updates corporations specified in
/// the update list, leaving other corporations' affiliations unchanged.
///
/// Expected: Ok with only specified corporation updated
#[tokio::test]
async fn does_not_affect_other_corporations() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    // Create alliances and corporations
    let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
    let corp1 = test
        .eve()
        .insert_mock_corporation(1, Some(alliance1.alliance_id), None)
        .await?;
    let corp2 = test
        .eve()
        .insert_mock_corporation(2, Some(alliance1.alliance_id), None)
        .await?;

    // Update only corp1
    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo
        .update_affiliations(vec![(corp1.id, Some(alliance2.id))])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify corp1 was updated
    let updated1 = entity::prelude::EveCorporation::find_by_id(corp1.id)
        .one(&test.db)
        .await?
        .expect("Corporation 1 should exist");
    assert_eq!(updated1.alliance_id, Some(alliance2.id));

    // Verify corp2 was NOT updated
    let updated2 = entity::prelude::EveCorporation::find_by_id(corp2.id)
        .one(&test.db)
        .await?
        .expect("Corporation 2 should exist");
    assert_eq!(
        updated2.alliance_id,
        Some(alliance1.id),
        "Corporation 2 should still have original alliance"
    );

    Ok(())
}

/// Tests handling mixed alliance assignments.
///
/// Verifies that the corporation repository correctly processes a batch containing
/// both Some and None alliance IDs, applying each appropriately.
///
/// Expected: Ok with corporations having correct alliance assignments
#[tokio::test]
async fn handles_mixed_alliance_assignments() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    // Create alliance
    let alliance = test.eve().insert_mock_alliance(1, None).await?;

    // Create corporations
    let corp1 = test.eve().insert_mock_corporation(1, None, None).await?;
    let corp2 = test.eve().insert_mock_corporation(2, None, None).await?;
    let corp3 = test.eve().insert_mock_corporation(3, None, None).await?;

    // Update with mixed alliance IDs
    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo
        .update_affiliations(vec![
            (corp1.id, Some(alliance.id)),
            (corp2.id, None),
            (corp3.id, Some(alliance.id)),
        ])
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);

    // Verify updates
    let updated1 = entity::prelude::EveCorporation::find_by_id(corp1.id)
        .one(&test.db)
        .await?
        .expect("Corporation 1 should exist");
    let updated2 = entity::prelude::EveCorporation::find_by_id(corp2.id)
        .one(&test.db)
        .await?
        .expect("Corporation 2 should exist");
    let updated3 = entity::prelude::EveCorporation::find_by_id(corp3.id)
        .one(&test.db)
        .await?
        .expect("Corporation 3 should exist");

    assert_eq!(updated1.alliance_id, Some(alliance.id));
    assert_eq!(updated2.alliance_id, None);
    assert_eq!(updated3.alliance_id, Some(alliance.id));

    Ok(())
}
