use super::*;

/// Expect Ok when upserting a new corporation with both alliance and faction
#[tokio::test]
async fn creates_new_corporation_with_alliance_and_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let faction_model = test.eve().insert_mock_faction(1).await?;
    let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
    let (corporation_id, corporation) = test.eve().with_mock_corporation(
        1,
        Some(alliance_model.alliance_id),
        Some(faction_model.faction_id),
    );

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert(
            corporation_id,
            corporation,
            Some(alliance_model.id),
            Some(faction_model.id),
        )
        .await;

    assert!(result.is_ok());
    let created = result.unwrap();
    let (corporation_id, corporation) = test.eve().with_mock_corporation(
        1,
        Some(alliance_model.alliance_id),
        Some(faction_model.faction_id),
    );
    assert_eq!(created.corporation_id, corporation_id);
    assert_eq!(created.name, corporation.name);
    assert_eq!(created.alliance_id, Some(alliance_model.id));
    assert_eq!(created.faction_id, Some(faction_model.id));

    Ok(())
}

/// Expect Ok when upserting a new corporation with only faction
#[tokio::test]
async fn creates_new_corporation_with_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let faction_model = test.eve().insert_mock_faction(1).await?;
    let (corporation_id, corporation) =
        test.eve()
            .with_mock_corporation(1, None, Some(faction_model.faction_id));

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert(corporation_id, corporation, None, Some(faction_model.id))
        .await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.alliance_id, None);
    assert_eq!(created.faction_id, Some(faction_model.id));

    Ok(())
}

/// Expect Ok when upserting a new corporation without alliance or faction
#[tokio::test]
async fn creates_new_corporation_without_alliance_or_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let (corporation_id, corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert(corporation_id, corporation, None, None)
        .await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.alliance_id, None);
    assert_eq!(created.faction_id, None);

    Ok(())
}

/// Expect Ok when upserting an existing corporation and verify it updates
#[tokio::test]
async fn updates_existing_corporation() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

    // Create updated corporation data with different values
    let (corporation_id, mut updated_corporation) = test.eve().with_mock_corporation(1, None, None);
    updated_corporation.name = "Updated Corporation Name".to_string();
    updated_corporation.ticker = "NEW".to_string();
    updated_corporation.member_count = 9999;

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert(corporation_id, updated_corporation, None, None)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    // Verify the ID remains the same (it's an update, not a new insert)
    assert_eq!(upserted.id, corporation_model.id);
    assert_eq!(upserted.corporation_id, corporation_model.corporation_id);
    assert_eq!(upserted.name, "Updated Corporation Name");
    assert_eq!(upserted.ticker, "NEW");
    assert_eq!(upserted.member_count, 9999);

    Ok(())
}

/// Expect Ok when upserting an existing corporation with a new alliance ID
#[tokio::test]
async fn updates_corporation_alliance_relationship() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let alliance_model1 = test.eve().insert_mock_alliance(1, None).await?;
    let alliance_model2 = test.eve().insert_mock_alliance(2, None).await?;
    let corporation_model = test
        .eve()
        .insert_mock_corporation(1, Some(alliance_model1.alliance_id), None)
        .await?;

    // Update corporation with new alliance
    let (corporation_id, corporation) =
        test.eve()
            .with_mock_corporation(1, Some(alliance_model2.alliance_id), None);

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert(corporation_id, corporation, Some(alliance_model2.id), None)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, corporation_model.id);
    assert_eq!(upserted.alliance_id, Some(alliance_model2.id));
    assert_ne!(upserted.alliance_id, Some(alliance_model1.id));

    Ok(())
}

/// Expect Ok when upserting an existing corporation with a new faction ID
#[tokio::test]
async fn updates_corporation_faction_relationship() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let faction_model1 = test.eve().insert_mock_faction(1).await?;
    let faction_model2 = test.eve().insert_mock_faction(2).await?;
    let corporation_model = test
        .eve()
        .insert_mock_corporation(1, None, Some(faction_model1.faction_id))
        .await?;

    // Update corporation with new faction
    let (corporation_id, corporation) =
        test.eve()
            .with_mock_corporation(1, None, Some(faction_model2.faction_id));

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert(corporation_id, corporation, None, Some(faction_model2.id))
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, corporation_model.id);
    assert_eq!(upserted.faction_id, Some(faction_model2.id));
    assert_ne!(upserted.faction_id, Some(faction_model1.id));

    Ok(())
}

/// Expect Ok when upserting removes alliance relationship
#[tokio::test]
async fn removes_alliance_relationship_on_upsert() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
    let corporation_model = test
        .eve()
        .insert_mock_corporation(1, Some(alliance_model.alliance_id), None)
        .await?;

    assert!(corporation_model.alliance_id.is_some());

    // Update corporation without alliance
    let (corporation_id, corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert(corporation_id, corporation, None, None)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, corporation_model.id);
    assert_eq!(upserted.alliance_id, None);

    Ok(())
}

/// Expect Ok when upserting removes faction relationship
#[tokio::test]
async fn removes_faction_relationship_on_upsert() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let faction_model = test.eve().insert_mock_faction(1).await?;
    let corporation_model = test
        .eve()
        .insert_mock_corporation(1, None, Some(faction_model.faction_id))
        .await?;

    assert!(corporation_model.faction_id.is_some());

    // Update corporation without faction
    let (corporation_id, corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert(corporation_id, corporation, None, None)
        .await;

    assert!(result.is_ok());
    let upserted = result.unwrap();

    assert_eq!(upserted.id, corporation_model.id);
    assert_eq!(upserted.faction_id, None);

    Ok(())
}

/// Expect Error when upserting to a table that doesn't exist
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!()?;
    let (corporation_id, corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .upsert(corporation_id, corporation, None, None)
        .await;

    assert!(result.is_err());

    Ok(())
}
