//! Tests for CorporationRepository::update_info_timestamp method.
//!
//! This module verifies the corporation info timestamp update behavior,
//! including updating existing corporations and handling non-existent corporations.

use super::*;

/// Tests updating info timestamp for an existing corporation.
///
/// Verifies that the corporation repository successfully updates the info_updated_at
/// timestamp to the current time for an existing corporation record.
///
/// Expected: Ok with updated corporation having newer info_updated_at timestamp
#[tokio::test]
async fn updates_info_timestamp_for_existing_corporation() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let (corporation_id, corporation) = test.eve().mock_corporation(1, None, None);

    let corporation_repo = CorporationRepository::new(&test.db);
    let created = corporation_repo
        .upsert_many(vec![(corporation_id, corporation, None, None)])
        .await?;

    let created_corp = &created[0];
    let initial_timestamp = created_corp.info_updated_at;

    // Wait a moment to ensure timestamp changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let result = corporation_repo
        .update_info_timestamp(created_corp.id)
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let updated_corp = result.unwrap();

    assert_eq!(updated_corp.id, created_corp.id);
    assert_eq!(updated_corp.corporation_id, corporation_id);
    assert!(
        updated_corp.info_updated_at > initial_timestamp,
        "info_updated_at should be newer: {:?} vs {:?}",
        updated_corp.info_updated_at,
        initial_timestamp
    );

    Ok(())
}

/// Tests updating info timestamp returns error for non-existent corporation.
///
/// Verifies that the corporation repository returns an error when attempting
/// to update the info timestamp for a corporation ID that doesn't exist.
///
/// Expected: Err(DbErr::RecordNotFound)
#[tokio::test]
async fn returns_error_for_nonexistent_corporation() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;

    let corporation_repo = CorporationRepository::new(&test.db);
    let result = corporation_repo.update_info_timestamp(999999).await;

    assert!(result.is_err());
    match result {
        Err(sea_orm::DbErr::RecordNotFound(_)) => (),
        _ => panic!("Expected DbErr::RecordNotFound, got: {:?}", result),
    }

    Ok(())
}

/// Tests that update_info_timestamp only updates the timestamp field.
///
/// Verifies that updating the info timestamp doesn't modify any other
/// corporation data fields like name, ticker, member count, etc.
///
/// Expected: Ok with all fields unchanged except info_updated_at
#[tokio::test]
async fn only_updates_timestamp_field() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let (corporation_id, corporation) = test.eve().mock_corporation(1, None, None);

    let corporation_repo = CorporationRepository::new(&test.db);
    let created = corporation_repo
        .upsert_many(vec![(corporation_id, corporation.clone(), None, None)])
        .await?;

    let created_corp = &created[0];

    // Wait a moment to ensure timestamp changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let updated_corp = corporation_repo
        .update_info_timestamp(created_corp.id)
        .await?;

    // Verify all fields except info_updated_at remain the same
    assert_eq!(updated_corp.id, created_corp.id);
    assert_eq!(updated_corp.corporation_id, created_corp.corporation_id);
    assert_eq!(updated_corp.name, created_corp.name);
    assert_eq!(updated_corp.ticker, created_corp.ticker);
    assert_eq!(updated_corp.ceo_id, created_corp.ceo_id);
    assert_eq!(updated_corp.creator_id, created_corp.creator_id);
    assert_eq!(updated_corp.member_count, created_corp.member_count);
    assert_eq!(updated_corp.tax_rate, created_corp.tax_rate);
    assert_eq!(updated_corp.alliance_id, created_corp.alliance_id);
    assert_eq!(updated_corp.faction_id, created_corp.faction_id);
    assert_eq!(updated_corp.created_at, created_corp.created_at);
    assert_eq!(
        updated_corp.affiliation_updated_at,
        created_corp.affiliation_updated_at
    );

    // Only info_updated_at should change
    assert!(updated_corp.info_updated_at > created_corp.info_updated_at);

    Ok(())
}

/// Tests updating info timestamp multiple times.
///
/// Verifies that the info timestamp can be updated multiple times and
/// each update results in a newer timestamp.
///
/// Expected: Ok with progressively newer timestamps on each update
#[tokio::test]
async fn handles_multiple_updates() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .with_table(entity::prelude::EveCorporation)
        .build()
        .await?;
    let (corporation_id, corporation) = test.eve().mock_corporation(1, None, None);

    let corporation_repo = CorporationRepository::new(&test.db);
    let created = corporation_repo
        .upsert_many(vec![(corporation_id, corporation, None, None)])
        .await?;

    let created_corp = &created[0];
    let mut previous_timestamp = created_corp.info_updated_at;

    // Update timestamp 3 times
    for _ in 0..3 {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let updated = corporation_repo
            .update_info_timestamp(created_corp.id)
            .await?;

        assert!(
            updated.info_updated_at > previous_timestamp,
            "Each update should result in a newer timestamp"
        );
        previous_timestamp = updated.info_updated_at;
    }

    Ok(())
}

/// Tests updating info timestamp for corporations with affiliations.
///
/// Verifies that updating the info timestamp works correctly for corporations
/// that have alliance and faction affiliations.
///
/// Expected: Ok with updated timestamp and preserved affiliations
#[tokio::test]
async fn updates_timestamp_for_corporation_with_affiliations() -> Result<(), TestError> {
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
    let created = corporation_repo
        .upsert_many(vec![(
            corporation_id,
            corporation,
            Some(alliance.id),
            Some(faction.id),
        )])
        .await?;

    let created_corp = &created[0];
    let initial_timestamp = created_corp.info_updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let updated = corporation_repo
        .update_info_timestamp(created_corp.id)
        .await?;

    assert!(updated.info_updated_at > initial_timestamp);
    assert_eq!(updated.alliance_id, Some(alliance.id));
    assert_eq!(updated.faction_id, Some(faction.id));

    Ok(())
}
