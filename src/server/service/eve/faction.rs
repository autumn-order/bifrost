use chrono::Utc;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::faction::FactionRepository, error::Error, util::time::effective_faction_cache_expiry,
};

pub struct FactionService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> FactionService<'a> {
    /// Creates a new instance of [`FactionService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Fetches & stores NPC faction information from ESI so long as they aren't within cache period
    ///
    /// The NPC faction cache expires at 11:05 UTC (after downtime)
    pub async fn update_factions(&self) -> Result<Vec<entity::eve_faction::Model>, Error> {
        let faction_repo = FactionRepository::new(self.db);

        let now = Utc::now();
        let effective_expiry = effective_faction_cache_expiry(now)?;

        // If the latest faction entry was updated at or after the effective expiry, skip updating.
        if let Some(faction) = faction_repo.get_latest().await? {
            if faction.updated_at >= effective_expiry {
                return Ok(Vec::new());
            }
        }

        let factions = self.esi_client.universe().get_factions().await?;

        let factions = faction_repo.upsert_many(factions).await?;

        Ok(factions)
    }

    /// Attempt to get a faction from the database using its EVE Online faction ID, attempt to update factions if faction is not found
    ///
    /// Faction updates will only occur once per 24 hour period which resets at 11:05 UTC when the EVE ESI
    /// faction cache expires.
    ///
    /// For simply getting a faction without an update, use [`FactionRepository::get_by_faction_id`]
    pub async fn get_or_update_factions(
        &self,
        faction_id: i64,
    ) -> Result<entity::eve_faction::Model, Error> {
        let faction_repo = FactionRepository::new(self.db);

        let result = faction_repo.get_by_faction_id(faction_id).await?;

        if let Some(faction) = result {
            return Ok(faction);
        };

        // If the faction is not found, then a new patch may have come out adding
        // a new faction. Attempt to update factions if they haven't already been
        // updated since downtime.
        let updated_factions = self.update_factions().await?;

        if let Some(faction) = updated_factions
            .into_iter()
            .find(|f| f.faction_id == faction_id)
        {
            return Ok(faction);
        }

        Err(Error::EveFactionNotFound(faction_id))
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use sea_orm::{
        ActiveModelTrait, ActiveValue, ConnectionTrait, DatabaseConnection, DbBackend, DbErr,
        IntoActiveModel, Schema,
    };

    use crate::server::{
        data::eve::faction::FactionRepository,
        util::test::{
            eve::mock::mock_faction,
            setup::{test_setup, TestSetup},
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

    async fn create_existing_faction_entry(
        db: &DatabaseConnection,
        updated_at: NaiveDateTime,
    ) -> Result<entity::eve_faction::Model, DbErr> {
        let faction_repo = FactionRepository::new(&db);

        let faction = mock_faction();

        faction_repo.upsert_many(vec![faction]).await?;

        let faction = faction_repo.get_latest().await?;
        let mut faction_am = faction.unwrap().into_active_model();

        faction_am.updated_at = ActiveValue::Set(updated_at);

        let faction = faction_am.update(db).await?;

        Ok(faction)
    }

    mod update_factions_tests {
        use chrono::{Duration, Utc};

        use crate::server::{
            error::Error,
            service::eve::faction::{
                tests::{create_existing_faction_entry, setup},
                FactionService,
            },
            util::{
                test::{
                    eve::mock::mock_faction, mockito::faction::mock_faction_endpoint,
                    setup::test_setup,
                },
                time::effective_faction_cache_expiry,
            },
        };

        /// Test successful faction creation when table is empty
        #[tokio::test]
        async fn test_update_factions_creation_success() {
            let mut test = setup().await.unwrap();
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);

            let expected_requests = 1;
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction()], expected_requests);

            let update_result = faction_service.update_factions().await;

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
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);

            let now = Utc::now();
            let effective_expiry = effective_faction_cache_expiry(now).unwrap();

            // Set updated_at to *before* the effective expiry so an update should be performed.
            let updated_at = effective_expiry
                .checked_sub_signed(Duration::minutes(5))
                .unwrap_or(effective_expiry);

            let _ = create_existing_faction_entry(&test.state.db, updated_at)
                .await
                .unwrap();
            let expected_requests = 1;
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction()], expected_requests);

            let update_result = faction_service.update_factions().await;

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
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);

            let now = Utc::now();
            let effective_expiry = effective_faction_cache_expiry(now).unwrap();

            // Set updated_at to just after the effective expiry so it should be considered cached.
            let updated_at = effective_expiry
                .checked_add_signed(Duration::minutes(1))
                .unwrap_or(effective_expiry);

            let _ = create_existing_faction_entry(&test.state.db, updated_at)
                .await
                .unwrap();

            // No requests should be made due to cache
            let expected_requests = 0;
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction()], expected_requests);

            let update_result = faction_service.update_factions().await;

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
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);

            let update_result = faction_service.update_factions().await;

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
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);

            // Function should error when attempting to get the latest faction entry from DB
            // due to the table not being created
            let update_result = faction_service.update_factions().await;

            assert!(
                update_result.is_err(),
                "Expected error, instead got: {:?}",
                update_result
            );

            assert!(matches!(update_result, Err(Error::DbErr(_))))
        }
    }

    mod get_or_update_factions_tests {
        use chrono::Utc;
        use sea_orm::DbErr;

        use crate::server::{
            error::Error,
            service::eve::faction::{
                tests::{create_existing_faction_entry, setup},
                FactionService,
            },
            util::test::{
                eve::mock::mock_faction, mockito::faction::mock_faction_endpoint, setup::test_setup,
            },
        };

        /// Expect faction to be found when present in database
        #[tokio::test]
        async fn test_get_or_update_factions_found() -> Result<(), DbErr> {
            let mut test = setup().await?;
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);

            let faction =
                create_existing_faction_entry(&test.state.db, Utc::now().naive_utc()).await?;

            let expected_requests = 0;
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction()], expected_requests);

            let result = faction_service
                .get_or_update_factions(faction.faction_id)
                .await;

            // Assert no requests were made to faction endpoint
            faction_endpoint.assert();

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect factions to be updated when not present in database
        #[tokio::test]
        async fn test_get_or_update_factions_update() -> Result<(), DbErr> {
            let mut test = setup().await?;
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);

            let expected_requests = 1;
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction()], expected_requests);

            // Database has no faction present, factions will be fetched & updated from endpoint
            let mock_faction = mock_faction();
            let update_result = faction_service
                .get_or_update_factions(mock_faction.faction_id)
                .await;

            assert!(update_result.is_ok());

            // Call method again, no additional endpoint requests should be made as it should be retrieved from DB
            let get_result = faction_service
                .get_or_update_factions(mock_faction.faction_id)
                .await;

            assert!(get_result.is_ok());

            // Assert only 1 request was made to faction endpoint
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect database error when table is not created
        #[tokio::test]
        async fn test_get_or_update_factions_database_error() -> Result<(), DbErr> {
            // Use shared test_setup util instead of specific setup() method for these tests
            // No table will be made for factions which will cause a database error
            let mut test = test_setup().await;
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);

            let expected_requests = 0;
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction()], expected_requests);

            let faction_id = 1;
            let result = faction_service.get_or_update_factions(faction_id).await;

            // Assert no requests were made to faction endpoint
            faction_endpoint.assert();

            assert!(result.is_err());

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        /// Expect ESI error when endpoint is not available
        #[tokio::test]
        async fn test_get_or_update_factions_esi_error() -> Result<(), DbErr> {
            let test = setup().await?;
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);

            // Don't create a mock faction endpoint, this will cause an ESI error when trying to
            // fetch faction

            let faction_id = 1;
            let result = faction_service.get_or_update_factions(faction_id).await;

            assert!(result.is_err());

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect EveFactionNotFound error if ESI does not return the required faction
        #[tokio::test]
        async fn test_get_or_update_factions_update_error() -> Result<(), DbErr> {
            let mut test = setup().await?;
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);

            let expected_requests = 1;
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction()], expected_requests);

            // Try to fetch a faction id that is not provided by the endpoint, will cause a not found error
            let faction_id = 1;
            let result = faction_service.get_or_update_factions(faction_id).await;

            // Assert 1 request was made to faction endpoint
            faction_endpoint.assert();

            assert!(result.is_err());

            assert!(matches!(result, Err(Error::EveFactionNotFound(_))));

            Ok(())
        }
    }
}
