//! Tests for AllianceRepository::update_info_timestamp method.
//!
//! This module verifies the alliance info timestamp update behavior,
//! including updating existing alliances and handling non-existent alliances.

use super::*;

/// Tests updating info timestamp for an existing alliance.
///
/// Verifies that the alliance repository successfully updates the updated_at
/// timestamp to the current time for an existing alliance record.
///
/// Expected: Ok with updated alliance having newer updated_at timestamp
#[tokio::test]
async fn updates_info_timestamp_for_existing_alliance() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let (alliance_id, alliance) = test.eve().mock_alliance(1, None);

    let alliance_repo = AllianceRepository::new(&test.db);
    let created = alliance_repo
        .upsert_many(vec![(alliance_id, alliance, None)])
        .await?;

    let created_alliance = &created[0];
    let initial_timestamp = created_alliance.updated_at;

    // Wait a moment to ensure timestamp changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let result = alliance_repo
        .update_info_timestamp(created_alliance.id)
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let updated_alliance = result.unwrap();

    assert_eq!(updated_alliance.id, created_alliance.id);
    assert_eq!(updated_alliance.alliance_id, alliance_id);
    assert!(
        updated_alliance.updated_at > initial_timestamp,
        "updated_at should be newer: {:?} vs {:?}",
        updated_alliance.updated_at,
        initial_timestamp
    );

    Ok(())
}

/// Tests updating info timestamp returns error for non-existent alliance.
///
/// Verifies that the alliance repository returns an error when attempting
/// to update the info timestamp for an alliance ID that doesn't exist.
///
/// Expected: Err(DbErr::RecordNotFound)
#[tokio::test]
async fn returns_error_for_nonexistent_alliance() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;

    let alliance_repo = AllianceRepository::new(&test.db);
    let result = alliance_repo.update_info_timestamp(999999).await;

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
/// alliance data fields like name, ticker, creator_id, etc.
///
/// Expected: Ok with all fields unchanged except updated_at
#[tokio::test]
async fn only_updates_timestamp_field() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let (alliance_id, alliance) = test.eve().mock_alliance(1, None);

    let alliance_repo = AllianceRepository::new(&test.db);
    let created = alliance_repo
        .upsert_many(vec![(alliance_id, alliance.clone(), None)])
        .await?;

    let created_alliance = &created[0];

    // Wait a moment to ensure timestamp changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let updated_alliance = alliance_repo
        .update_info_timestamp(created_alliance.id)
        .await?;

    // Verify all fields except updated_at remain the same
    assert_eq!(updated_alliance.id, created_alliance.id);
    assert_eq!(updated_alliance.alliance_id, created_alliance.alliance_id);
    assert_eq!(updated_alliance.name, created_alliance.name);
    assert_eq!(updated_alliance.ticker, created_alliance.ticker);
    assert_eq!(updated_alliance.creator_id, created_alliance.creator_id);
    assert_eq!(
        updated_alliance.creator_corporation_id,
        created_alliance.creator_corporation_id
    );
    assert_eq!(
        updated_alliance.executor_corporation_id,
        created_alliance.executor_corporation_id
    );
    assert_eq!(updated_alliance.date_founded, created_alliance.date_founded);
    assert_eq!(updated_alliance.faction_id, created_alliance.faction_id);
    assert_eq!(updated_alliance.created_at, created_alliance.created_at);

    // Only updated_at should change
    assert!(updated_alliance.updated_at > created_alliance.updated_at);

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
        .build()
        .await?;
    let (alliance_id, alliance) = test.eve().mock_alliance(1, None);

    let alliance_repo = AllianceRepository::new(&test.db);
    let created = alliance_repo
        .upsert_many(vec![(alliance_id, alliance, None)])
        .await?;

    let created_alliance = &created[0];
    let mut previous_timestamp = created_alliance.updated_at;

    // Update timestamp 3 times
    for _ in 0..3 {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let updated = alliance_repo
            .update_info_timestamp(created_alliance.id)
            .await?;

        assert!(
            updated.updated_at > previous_timestamp,
            "Each update should result in a newer timestamp"
        );
        previous_timestamp = updated.updated_at;
    }

    Ok(())
}

/// Tests updating info timestamp for alliances with faction.
///
/// Verifies that updating the info timestamp works correctly for alliances
/// that have faction affiliations.
///
/// Expected: Ok with updated timestamp and preserved faction
#[tokio::test]
async fn updates_timestamp_for_alliance_with_faction() -> Result<(), TestError> {
    let mut test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let faction = test.eve().insert_mock_faction(1).await?;
    let (alliance_id, alliance) = test.eve().mock_alliance(1, Some(1));

    let alliance_repo = AllianceRepository::new(&test.db);
    let created = alliance_repo
        .upsert_many(vec![(alliance_id, alliance, Some(faction.id))])
        .await?;

    let created_alliance = &created[0];
    let initial_timestamp = created_alliance.updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let updated = alliance_repo
        .update_info_timestamp(created_alliance.id)
        .await?;

    assert!(updated.updated_at > initial_timestamp);
    assert_eq!(updated.faction_id, Some(faction.id));

    Ok(())
}
