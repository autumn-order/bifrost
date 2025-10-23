use chrono::Utc;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::faction::FactionRepository, error::Error, util::time::effective_faction_cache_expiry,
};

/// Fetches & stores NPC faction information from ESI so long as they aren't within cache period
///
/// The NPC faction cache expires at 11:05 UTC (after downtime)
pub async fn update_factions(
    db: &DatabaseConnection,
    esi_client: &eve_esi::Client,
) -> Result<Vec<entity::eve_faction::Model>, Error> {
    let faction_repo = FactionRepository::new(&db);

    let now = Utc::now();
    let effective_expiry = effective_faction_cache_expiry(now)?;

    // If the latest faction entry was updated at or after the effective expiry, skip updating.
    if let Some(faction) = faction_repo.get_latest().await? {
        if faction.updated_at >= effective_expiry {
            return Ok(Vec::new());
        }
    }

    let factions = esi_client.universe().get_factions().await?;

    let factions = faction_repo.upsert_many(factions).await?;

    Ok(factions)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDateTime, Utc};
    use mockito::{Mock, ServerGuard};
    use sea_orm::{
        ActiveModelTrait, ActiveValue, ConnectionTrait, DatabaseConnection, DbBackend, DbErr,
        IntoActiveModel, Schema,
    };

    use crate::server::{
        data::eve::faction::FactionRepository,
        error::Error,
        service::eve::faction::update_factions,
        util::{
            test::{
                eve::mock::mock_faction,
                setup::{test_setup, TestSetup},
            },
            time::effective_faction_cache_expiry,
        },
    };

    async fn setup() -> Result<TestSetup, DbErr> {
        let test = test_setup().await;

        let db = &test.state.db;
        let schema = Schema::new(DbBackend::Sqlite);

        let stmt = schema.create_table_from_entity(entity::prelude::EveFaction);

        db.execute(&stmt).await?;

        Ok(test)
    }

    async fn create_existing_faction_entry(db: &DatabaseConnection, updated_at: NaiveDateTime) {
        let faction_repo = FactionRepository::new(&db);

        let faction = mock_faction();

        faction_repo.upsert_many(vec![faction]).await.unwrap();

        let faction = faction_repo.get_latest().await.unwrap();
        let mut faction_am = faction.unwrap().into_active_model();

        faction_am.updated_at = ActiveValue::Set(updated_at);

        faction_am.update(db).await.unwrap();
    }

    /// Mock endpoint representing the ESI faction endpoint
    fn mock_faction_endpoint(server: &mut ServerGuard, expected_requests: usize) -> Mock {
        let faction = mock_faction();

        server
            .mock("GET", "/universe/factions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&vec![faction]).unwrap())
            .expect(expected_requests)
            .create()
    }

    /// Test successful faction creation when table is empty
    #[tokio::test]
    async fn test_update_factions_creation_success() {
        let mut test = setup().await.unwrap();

        let expected_requests = 1;
        let faction_endpoint = mock_faction_endpoint(&mut test.server, expected_requests);

        let update_result = update_factions(&test.state.db, &test.state.esi_client).await;

        // Assert a request was made to mock endpoint
        faction_endpoint.assert();

        assert!(update_result.is_ok());
        let updated = update_result.unwrap();

        assert!(!updated.is_empty())
    }

    /// Test successful faction creation when table has existing entries
    #[tokio::test]
    async fn test_update_factions_existing_entries_success() {
        let mut test = setup().await.unwrap();

        let now = Utc::now();
        let effective_expiry = effective_faction_cache_expiry(now).unwrap();

        // Set updated_at to *before* the effective expiry so an update should be performed.
        let updated_at = effective_expiry
            .checked_sub_signed(Duration::minutes(5))
            .unwrap_or(effective_expiry);

        create_existing_faction_entry(&test.state.db, updated_at).await;

        let expected_requests = 1;
        let faction_endpoint = mock_faction_endpoint(&mut test.server, expected_requests);

        let update_result = update_factions(&test.state.db, &test.state.esi_client).await;

        // Assert a request was made to mock endpoint
        faction_endpoint.assert();

        assert!(update_result.is_ok());
        let updated = update_result.unwrap();

        // Assert list of updated factions is not empty
        assert!(!updated.is_empty())
    }

    /// Test no update performed due to still being within cache period
    #[tokio::test]
    async fn test_update_factions_cached() {
        let mut test = setup().await.unwrap();

        let now = Utc::now();
        let effective_expiry = effective_faction_cache_expiry(now).unwrap();

        // Set updated_at to just after the effective expiry so it should be considered cached.
        let updated_at = effective_expiry
            .checked_add_signed(Duration::minutes(1))
            .unwrap_or(effective_expiry);

        create_existing_faction_entry(&test.state.db, updated_at).await;

        // No requests should be made due to cache
        let expected_requests = 0;
        let faction_endpoint = mock_faction_endpoint(&mut test.server, expected_requests);

        let update_result = update_factions(&test.state.db, &test.state.esi_client).await;

        // Assert no request was made to mock endpoint
        faction_endpoint.assert();

        assert!(update_result.is_ok());
        let updated = update_result.unwrap();

        // Should be empty since no updates were made
        assert!(updated.is_empty())
    }

    /// Test failed faction update due to ESI error
    #[tokio::test]
    async fn test_update_factions_esi_error() {
        let test = setup().await.unwrap();

        let update_result = update_factions(&test.state.db, &test.state.esi_client).await;

        assert!(
            update_result.is_err(),
            "Expected error, instead got: {:?}",
            update_result
        );

        assert!(matches!(
            update_result,
            Err(Error::EsiError(eve_esi::Error::ReqwestError(_)))
        ))
    }

    /// Test failed faction update due to database error
    #[tokio::test]
    async fn test_update_factions_database_error() {
        let test = test_setup().await;

        // Function should error when attempting to get the latest faction entry from DB
        // due to the table not being created
        let update_result = update_factions(&test.state.db, &test.state.esi_client).await;

        assert!(
            update_result.is_err(),
            "Expected error, instead got: {:?}",
            update_result
        );

        assert!(matches!(update_result, Err(Error::DbErr(_))))
    }
}
