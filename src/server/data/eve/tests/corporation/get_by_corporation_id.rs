use super::*;

/// Expect Some when getting corporation present in table
#[tokio::test]
async fn finds_existing_corporation() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;
    let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

    let corporation_repo = CorporationRepository::new(&test.state.db);
    let result = corporation_repo
        .get_by_corporation_id(corporation_model.corporation_id)
        .await;

    assert!(result.is_ok());
    let corporation_option = result.unwrap();
    assert!(corporation_option.is_some());

    Ok(())
}

/// Expect None when getting corporation not present in table
#[tokio::test]
async fn returns_none_for_nonexistent_corporation() -> Result<(), TestError> {
    let test = test_setup_with_tables!(
        entity::prelude::EveFaction,
        entity::prelude::EveAlliance,
        entity::prelude::EveCorporation
    )?;

    let corporation_repo = CorporationRepository::new(&test.state.db);
    let corporation_id = 1;
    let result = corporation_repo.get_by_corporation_id(corporation_id).await;

    assert!(result.is_ok());
    let corporation_option = result.unwrap();
    assert!(corporation_option.is_none());

    Ok(())
}

/// Expect Error when required tables haven't been created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;

    let corporation_repo = CorporationRepository::new(&test.state.db);
    let corporation_id = 1;
    let result = corporation_repo.get_by_corporation_id(corporation_id).await;

    assert!(result.is_err());

    Ok(())
}
