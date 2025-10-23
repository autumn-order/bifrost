use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::corporation::CorporationRepository,
    error::Error,
    service::eve::{alliance::AllianceService, faction::FactionService},
};

pub struct CorporationService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CorporationService<'a> {
    /// Creates a new instance of [`CorporationService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Fetches a corporation from EVE Online's ESI and creates a database entry
    pub async fn create_corporation(
        &self,
        corporation_id: i64,
    ) -> Result<entity::eve_corporation::Model, Error> {
        let corporation_repo = CorporationRepository::new(&self.db);
        let alliance_service = AllianceService::new(&self.db, &self.esi_client);
        let faction_service = FactionService::new(&self.db, &self.esi_client);

        let corporation = self
            .esi_client
            .corporation()
            .get_corporation_information(corporation_id)
            .await?;

        let alliance_id = match corporation.alliance_id {
            Some(id) => Some(alliance_service.get_or_create_alliance(id).await?.id),
            None => None,
        };

        let faction_id = match corporation.faction_id {
            Some(id) => Some(faction_service.get_or_update_factions(id).await?.id),
            None => None,
        };

        let corporation = corporation_repo
            .create(corporation_id, corporation, alliance_id, faction_id)
            .await?;

        Ok(corporation)
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, DbBackend, DbErr, Schema};

    use crate::server::util::test::setup::{test_setup, TestSetup};

    /// Creates prerequisite corporation, alliance, & faction tables for tests
    async fn setup() -> Result<TestSetup, DbErr> {
        let test = test_setup().await;

        let db = &test.state.db;
        let schema = Schema::new(DbBackend::Sqlite);

        let stmts = vec![
            schema.create_table_from_entity(entity::prelude::EveFaction),
            schema.create_table_from_entity(entity::prelude::EveAlliance),
            schema.create_table_from_entity(entity::prelude::EveCorporation),
        ];

        for stmt in stmts {
            db.execute(&stmt).await?;
        }

        Ok(test)
    }

    mod create_corporation_tests {
        use sea_orm::DbErr;

        use crate::server::{
            error::Error,
            service::eve::corporation::{tests::setup, CorporationService},
            util::test::{
                eve::mock::{mock_alliance, mock_corporation, mock_faction},
                mockito::{
                    alliance::mock_alliance_endpoint, corporation::mock_corporation_endpoint,
                    faction::mock_faction_endpoint,
                },
                setup::test_setup,
            },
        };

        /// Expect success when creating corporation with no alliance or faction
        #[tokio::test]
        async fn test_create_corporation_success() -> Result<(), DbErr> {
            let mut test = setup().await?;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);

            let alliance_id = None;
            let faction_id = None;
            let mock_corporation = mock_corporation(alliance_id, faction_id);

            let expected_requests = 1;
            let corporation_endpoint = mock_corporation_endpoint(
                &mut test.server,
                "/corporations/1",
                mock_corporation,
                expected_requests,
            );

            let corporation_id = 1;
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());

            // Assert 1 request was made to mock endpoint
            corporation_endpoint.assert();

            Ok(())
        }

        /// Expect success when creating corporation with alliance
        #[tokio::test]
        async fn test_create_corporation_success_with_alliance() -> Result<(), DbErr> {
            let mut test = setup().await?;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);

            let alliance_id = Some(1);
            let faction_id = None;
            let mock_corporation = mock_corporation(alliance_id, faction_id);
            let mock_alliance = mock_alliance(faction_id);

            let expected_requests = 1;
            let corporation_endpoint = mock_corporation_endpoint(
                &mut test.server,
                "/corporations/1",
                mock_corporation,
                expected_requests,
            );
            let alliance_endpoint = mock_alliance_endpoint(
                &mut test.server,
                "/alliances/1",
                mock_alliance,
                expected_requests,
            );

            let corporation_id = 1;
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());

            // Assert 1 request was made to each mock endpoint
            corporation_endpoint.assert();
            alliance_endpoint.assert();

            Ok(())
        }

        /// Expect success when creating corporation with faction
        #[tokio::test]
        async fn test_create_corporation_success_with_faction() -> Result<(), DbErr> {
            let mut test = setup().await?;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);

            let alliance_id = None;
            let faction_id = Some(0);
            let mock_corporation = mock_corporation(alliance_id, faction_id);
            let mock_faction = mock_faction();

            let expected_requests = 1;
            let corporation_endpoint = mock_corporation_endpoint(
                &mut test.server,
                "/corporations/1",
                mock_corporation,
                expected_requests,
            );
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction], expected_requests);

            let corporation_id = 1;
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());

            // Assert 1 request was made to each mock endpoint
            corporation_endpoint.assert();
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect success when creating corporation with alliance and faction
        #[tokio::test]
        async fn test_create_corporation_success_with_alliance_and_faction() -> Result<(), DbErr> {
            let mut test = setup().await?;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);

            let alliance_id = Some(1);
            let faction_id = Some(0);
            let mock_corporation = mock_corporation(alliance_id, faction_id);
            let mock_alliance = mock_alliance(faction_id);
            let mock_faction = mock_faction();

            let expected_requests = 1;
            let corporation_endpoint = mock_corporation_endpoint(
                &mut test.server,
                "/corporations/1",
                mock_corporation,
                expected_requests,
            );
            let alliance_endpoint = mock_alliance_endpoint(
                &mut test.server,
                "/alliances/1",
                mock_alliance,
                expected_requests,
            );
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction], expected_requests);

            let corporation_id = 1;
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok(), "Error: {:#?}", result);

            // Assert 1 request was made to each mock endpoint
            corporation_endpoint.assert();
            alliance_endpoint.assert();
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Error when fetch request to mock ESI endpoint fails
        #[tokio::test]
        async fn test_create_corporation_esi_error() -> Result<(), DbErr> {
            let test = setup().await?;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);

            // Create no mock ESI endpoints which will cause an error when fetching corporation

            let corporation_id = 1;
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error when database table is not created
        #[tokio::test]
        async fn test_create_corporation_database_error() -> Result<(), DbErr> {
            // Use setup function that doesn't create required tables to cause an error
            let mut test = test_setup().await;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);

            let alliance_id = None;
            let faction_id = None;
            let mock_corporation = mock_corporation(alliance_id, faction_id);

            let expected_requests = 1;
            let corporation_endpoint = mock_corporation_endpoint(
                &mut test.server,
                "/corporations/1",
                mock_corporation,
                expected_requests,
            );

            let corporation_id = 1;
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::DbErr(_))));

            // Assert 1 request was made to mock endpoint
            corporation_endpoint.assert();

            Ok(())
        }
    }
}
