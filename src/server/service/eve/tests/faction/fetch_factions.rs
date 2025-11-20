use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};

use crate::server::{
    error::Error, service::eve::faction::FactionService, util::time::effective_faction_cache_expiry,
};

use super::*;

/// Expect Some with factions fetched when table is empty
#[tokio::test]
async fn fetches_factions_when_table_empty() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    let faction_service = FactionService::new(test.state.db.clone(), test.state.esi_client.clone());
    let mut retry_cache = None;
    let result = faction_service.fetch_factions(&mut retry_cache).await;

    assert!(result.is_ok());
    let data = result.unwrap();
    assert!(data.is_some());
    let (factions, from_cache) = data.unwrap();
    assert_eq!(factions.len(), 1);
    assert_eq!(factions[0].faction_id, faction_id);
    assert!(!from_cache);
    assert!(retry_cache.is_some());

    faction_endpoint.assert();

    Ok(())
}

/// Expect Some with factions fetched when existing factions are past cache expiry
#[tokio::test]
async fn fetches_factions_past_cache_expiry() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
    let faction_model = test.eve().insert_mock_faction(1).await?;

    let mock_faction = test.eve().with_mock_faction(faction_model.faction_id);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    // Set updated_at to before the effective expiry
    let now = Utc::now();
    let effective_expiry = effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_sub_signed(Duration::minutes(5))
        .unwrap_or(effective_expiry);
    let mut faction_am = faction_model.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.state.db).await?;

    let faction_service = FactionService::new(test.state.db.clone(), test.state.esi_client.clone());
    let mut retry_cache = None;
    let result = faction_service.fetch_factions(&mut retry_cache).await;

    assert!(result.is_ok());
    let data = result.unwrap();
    assert!(data.is_some());
    let (factions, from_cache) = data.unwrap();
    assert_eq!(factions.len(), 1);
    assert!(!from_cache);
    assert!(retry_cache.is_some());

    faction_endpoint.assert();

    Ok(())
}

/// Expect None when factions are within cache expiry
#[tokio::test]
async fn returns_none_within_cache_expiry() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
    let faction_model = test.eve().insert_mock_faction(1).await?;

    let mock_faction = test.eve().with_mock_faction(faction_model.faction_id);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 0);

    // Set updated_at to after the effective expiry
    let now = Utc::now();
    let effective_expiry = effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_add_signed(Duration::minutes(1))
        .unwrap_or(effective_expiry);
    let mut faction_am = faction_model.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.state.db).await?;

    let faction_service = FactionService::new(test.state.db.clone(), test.state.esi_client.clone());
    let mut retry_cache = None;
    let result = faction_service.fetch_factions(&mut retry_cache).await;

    assert!(result.is_ok());
    let data = result.unwrap();
    assert!(data.is_none());
    assert!(retry_cache.is_none());

    faction_endpoint.assert();

    Ok(())
}

/// Expect Some with factions from retry_cache when cache is populated
#[tokio::test]
async fn returns_from_retry_cache_when_available() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let faction_endpoint = test
        .eve()
        .with_faction_endpoint(vec![mock_faction.clone()], 0);

    // Populate retry_cache with factions
    let faction_service = FactionService::new(test.state.db.clone(), test.state.esi_client.clone());
    let mut retry_cache = Some(vec![mock_faction]);

    let result = faction_service.fetch_factions(&mut retry_cache).await;

    assert!(result.is_ok());
    let data = result.unwrap();
    assert!(data.is_some());
    let (factions, from_cache) = data.unwrap();
    assert_eq!(factions.len(), 1);
    assert_eq!(factions[0].faction_id, faction_id);
    assert!(from_cache);

    // ESI should not be called
    faction_endpoint.assert();

    Ok(())
}

/// Expect Some with retry_cache populated after first ESI fetch
#[tokio::test]
async fn populates_retry_cache_on_first_fetch() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    let faction_service = FactionService::new(test.state.db.clone(), test.state.esi_client.clone());
    let mut retry_cache = None;
    let result = faction_service.fetch_factions(&mut retry_cache).await;

    assert!(result.is_ok());
    assert!(retry_cache.is_some());
    let cached_factions = retry_cache.unwrap();
    assert_eq!(cached_factions.len(), 1);
    assert_eq!(cached_factions[0].faction_id, faction_id);

    faction_endpoint.assert();

    Ok(())
}

/// Expect Error when ESI is unavailable
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    // No mock endpoint created - connection will fail
    let faction_service = FactionService::new(test.state.db.clone(), test.state.esi_client.clone());
    let mut retry_cache = None;
    let result = faction_service.fetch_factions(&mut retry_cache).await;

    assert!(matches!(
        result,
        Err(Error::EsiError(eve_esi::Error::ReqwestError(_)))
    ));

    Ok(())
}

/// Expect Error when database tables are missing
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!()?;

    let faction_id = 1;
    let mock_faction = test.eve().with_mock_faction(faction_id);
    let _faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

    let faction_service = FactionService::new(test.state.db.clone(), test.state.esi_client.clone());
    let mut retry_cache = None;
    let result = faction_service.fetch_factions(&mut retry_cache).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Expect retry_cache remains unchanged when factions are within cache expiry
#[tokio::test]
async fn retry_cache_unchanged_within_cache_expiry() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
    let faction_model = test.eve().insert_mock_faction(1).await?;

    // Set updated_at to within cache period
    let now = Utc::now();
    let effective_expiry = effective_faction_cache_expiry(now).unwrap();
    let updated_at = effective_expiry
        .checked_add_signed(Duration::minutes(1))
        .unwrap_or(effective_expiry);
    let mut faction_am = faction_model.into_active_model();
    faction_am.updated_at = ActiveValue::Set(updated_at);
    faction_am.update(&test.state.db).await?;

    let mock_faction = test.eve().with_mock_faction(1);
    let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 0);

    // Start with empty retry_cache
    let faction_service = FactionService::new(test.state.db.clone(), test.state.esi_client.clone());
    let mut retry_cache = None;
    let result = faction_service.fetch_factions(&mut retry_cache).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    assert!(retry_cache.is_none()); // Should remain None

    faction_endpoint.assert();

    Ok(())
}

/// Expect retry_cache to be reused on subsequent calls without ESI fetch
#[tokio::test]
async fn reuses_retry_cache_on_multiple_calls() -> Result<(), TestError> {
    let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

    let faction_id_1 = 1;
    let faction_id_2 = 2;
    let mock_faction_1 = test.eve().with_mock_faction(faction_id_1);
    let mock_faction_2 = test.eve().with_mock_faction(faction_id_2);
    let faction_endpoint = test
        .eve()
        .with_faction_endpoint(vec![mock_faction_1, mock_faction_2], 0);

    // Pre-populate retry_cache
    let cached_faction_1 = test.eve().with_mock_faction(faction_id_1);
    let cached_faction_2 = test.eve().with_mock_faction(faction_id_2);
    let mut retry_cache = Some(vec![cached_faction_1, cached_faction_2]);

    let faction_service = FactionService::new(test.state.db.clone(), test.state.esi_client.clone());

    // First call should use retry_cache
    let result1 = faction_service.fetch_factions(&mut retry_cache).await;

    assert!(result1.is_ok());
    let data1 = result1.unwrap();
    assert!(data1.is_some());
    let (factions1, from_cache1) = data1.unwrap();
    assert_eq!(factions1.len(), 2);
    assert!(from_cache1);

    // Second call should also use the same retry_cache
    let result2 = faction_service.fetch_factions(&mut retry_cache).await;

    assert!(result2.is_ok());
    let data2 = result2.unwrap();
    assert!(data2.is_some());
    let (factions2, from_cache2) = data2.unwrap();
    assert_eq!(factions2.len(), 2);
    assert!(from_cache2);

    // ESI should not be called at all
    faction_endpoint.assert();

    Ok(())
}
