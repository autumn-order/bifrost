use super::*;

// Expect Ok when inserting a corporation with both an alliance & faction ID
#[tokio::test]
async fn creates_corporation_with_alliance_and_faction() -> Result<(), TestError> {
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
        .create(
            corporation_id,
            corporation,
            Some(alliance_model.id),
            Some(faction_model.id),
        )
        .await;

    assert!(result.is_ok(), "Error: {:?}", result);
    let created = result.unwrap();
    let (corporation_id, corporation) = test.eve().with_mock_corporation(
        1,
        Some(alliance_model.alliance_id),
        Some(faction_model.faction_id),
    );
    assert_eq!(created.corporation_id, corporation_id,);
    assert_eq!(created.name, corporation.name);
    assert_eq!(created.alliance_id, Some(alliance_model.id),);
    assert_eq!(created.faction_id, Some(faction_model.id));

    Ok(())
}

/// Expect Ok when inserting a corporation with only a faction ID
#[tokio::test]
async fn creates_corporation_with_faction() -> Result<(), TestError> {
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
        .create(corporation_id, corporation, None, Some(faction_model.id))
        .await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.alliance_id, None);
    assert_eq!(created.faction_id, Some(faction_model.id));

    Ok(())
}

/// Should succeed when inserting corporation into table without a faction or alliance ID
#[tokio::test]
async fn creates_corporation_without_alliance_or_faction() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let (corporation_id, corporation) = test.eve().with_mock_corporation(1, None, None);

    let corporation_repo = CorporationRepository::new(test.state.db.clone());
    let result = corporation_repo
        .create(corporation_id, corporation, None, None)
        .await;

    assert!(result.is_ok());
    let created = result.unwrap();
    assert_eq!(created.alliance_id, None);
    assert_eq!(created.faction_id, None);

    Ok(())
}
