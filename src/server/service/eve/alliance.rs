use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::alliance::AllianceRepository, error::Error, service::eve::faction::FactionService,
};

pub struct AllianceService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> AllianceService<'a> {
    /// Creates a new instance of [`AllianceService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Fetches an alliance from EVE Online's ESI and creates a database entry
    pub async fn create_alliance(
        &self,
        alliance_id: i64,
    ) -> Result<entity::eve_alliance::Model, Error> {
        let alliance_repo = AllianceRepository::new(&self.db);
        let faction_service = FactionService::new(&self.db, &self.esi_client);

        let alliance = self
            .esi_client
            .alliance()
            .get_alliance_information(alliance_id)
            .await?;

        let faction_id = match alliance.faction_id {
            Some(id) => Some(faction_service.get_or_update_factions(id).await?.id),
            None => None,
        };

        let alliance = alliance_repo
            .create(alliance_id, alliance, faction_id)
            .await?;

        Ok(alliance)
    }

    /// Gets an alliance from database or creates an entry for it from ESI
    pub async fn get_or_create_alliance(
        &self,
        alliance_id: i64,
    ) -> Result<entity::eve_alliance::Model, Error> {
        let alliance_repo = AllianceRepository::new(&self.db);

        if let Some(alliance) = alliance_repo.get_by_alliance_id(alliance_id).await? {
            return Ok(alliance);
        }

        let alliance = self.create_alliance(alliance_id).await?;

        Ok(alliance)
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, DbBackend, DbErr, Schema};

    use crate::server::util::test::setup::{test_setup, TestSetup};

    /// Creates prerequisite faction & alliance tables for tests
    async fn setup() -> Result<TestSetup, DbErr> {
        let test = test_setup().await;

        let db = &test.state.db;
        let schema = Schema::new(DbBackend::Sqlite);

        let stmts = vec![
            schema.create_table_from_entity(entity::prelude::EveFaction),
            schema.create_table_from_entity(entity::prelude::EveAlliance),
        ];

        for stmt in stmts {
            db.execute(&stmt).await?;
        }

        Ok(test)
    }

    mod create_alliance_tests {
        use sea_orm::DbErr;

        use crate::server::{
            error::Error,
            service::eve::alliance::{tests::setup, AllianceService},
            util::test::{
                eve::mock::{mock_alliance, mock_faction},
                mockito::{alliance::mock_alliance_endpoint, faction::mock_faction_endpoint},
                setup::test_setup,
            },
        };

        /// Expect success when fetching & saving a new alliance to database
        #[tokio::test]
        async fn test_create_alliance_success() -> Result<(), DbErr> {
            let mut test = setup().await?;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);

            let mock_faction = mock_faction();
            let mock_faction_id = Some(mock_faction.faction_id);

            let expected_requests = 1;
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction], expected_requests);
            let alliance_endpoint = mock_alliance_endpoint(
                &mut test.server,
                "/alliances/1",
                mock_alliance(mock_faction_id),
                expected_requests,
            );

            let alliance_id = 1;
            let result = alliance_service.create_alliance(alliance_id).await;

            assert!(result.is_ok());

            // Assert 1 request was made to each mock endpoint
            faction_endpoint.assert();
            alliance_endpoint.assert();

            Ok(())
        }

        /// Expect success when fetching & saving a new alliance to database without a faction ID
        #[tokio::test]
        async fn test_create_alliance_success_no_faction() -> Result<(), DbErr> {
            let mut test = setup().await?;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);

            let expected_requests = 0;
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction()], expected_requests);

            let expected_requests = 1;
            let alliance_endpoint = mock_alliance_endpoint(
                &mut test.server,
                "/alliances/1",
                mock_alliance(None),
                expected_requests,
            );

            let alliance_id = 1;
            let result = alliance_service.create_alliance(alliance_id).await;

            assert!(result.is_ok());

            // Assert 0 requests were made to mock faction endpoint
            faction_endpoint.assert();

            // Assert 1 request was made to mock alliance endpoint
            alliance_endpoint.assert();

            Ok(())
        }

        /// Expect error when ESI fetch request fails
        #[tokio::test]
        async fn test_create_alliance_esi_error() -> Result<(), DbErr> {
            let test = setup().await?;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);

            // Do not create any mock ESI endpoints which will cause an ESI error

            let alliance_id = 1;
            let result = alliance_service.create_alliance(alliance_id).await;

            assert!(result.is_err());

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect error when trying to insert an already existing alliance into database
        #[tokio::test]
        async fn test_create_alliance_database_error() -> Result<(), DbErr> {
            // Use setup function that doesn't create the required tables to cause DB error
            let mut test = test_setup().await;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);

            let expected_requests = 1;
            let alliance_endpoint = mock_alliance_endpoint(
                &mut test.server,
                "/alliances/1",
                mock_alliance(None),
                expected_requests,
            );

            let alliance_id = 1;
            let result = alliance_service.create_alliance(alliance_id).await;

            assert!(result.is_err());

            // Assert 1 request was made to mock alliance endpoint, DB error occurs afterwards
            alliance_endpoint.assert();

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }
    }

    mod get_or_create_alliance_tests {
        use sea_orm::DbErr;

        use crate::server::{
            data::eve::alliance::AllianceRepository,
            error::Error,
            service::eve::alliance::{tests::setup, AllianceService},
            util::test::{
                eve::mock::mock_alliance, mockito::alliance::mock_alliance_endpoint,
                setup::test_setup,
            },
        };

        // Expect success when getting existing alliance
        #[tokio::test]
        async fn test_get_or_create_alliance_found() -> Result<(), DbErr> {
            let mut test = setup().await.unwrap();
            let alliance_repo = AllianceRepository::new(&test.state.db);
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);

            let expected_requests = 0;
            let alliance_endpoint = mock_alliance_endpoint(
                &mut test.server,
                "/alliances/1",
                mock_alliance(None),
                expected_requests,
            );

            // Insert existing alliance into database
            let alliance_id = 1;
            let faction_id = None;
            let alliance = mock_alliance(faction_id);
            alliance_repo.create(alliance_id, alliance, None).await?;

            let result = alliance_service.get_or_create_alliance(alliance_id).await;

            assert!(result.is_ok());

            // Assert no requests were made to alliance endpoint
            alliance_endpoint.assert();

            Ok(())
        }

        // Expect success when creating a new alliance
        #[tokio::test]
        async fn test_get_or_create_alliance_created() -> Result<(), DbErr> {
            let mut test = setup().await.unwrap();
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);

            let expected_requests = 1;
            let alliance_endpoint = mock_alliance_endpoint(
                &mut test.server,
                "/alliances/1",
                mock_alliance(None),
                expected_requests,
            );

            let alliance_id = 1;
            let result = alliance_service.get_or_create_alliance(alliance_id).await;

            assert!(result.is_ok());

            // Assert 1 request was made to alliance endpoint
            alliance_endpoint.assert();

            Ok(())
        }

        // Expect Error when getting an alliance from a table that does not exist
        #[tokio::test]
        async fn test_get_or_create_alliance_database_error() -> Result<(), DbErr> {
            // Use setup function that does not create required tables to cause DB error
            let test = test_setup().await;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);

            // No endpoint needed as function should fail before any fetch request is considered

            let alliance_id = 1;
            let result = alliance_service.get_or_create_alliance(alliance_id).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        // Expect Error when request to ESI endpoint fails
        #[tokio::test]
        async fn test_get_or_create_alliance_esi_error() -> Result<(), DbErr> {
            let test = setup().await.unwrap();
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);

            // Don't create an alliance endpoint, this will cause an ESI error when trying to fetch alliance

            let alliance_id = 1;
            let result = alliance_service.get_or_create_alliance(alliance_id).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }
    }
}
