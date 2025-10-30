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

    mod create_alliance {
        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::eve::alliance::AllianceService};

        /// Expect Ok when fetching & creating an alliance with a faction ID
        #[tokio::test]
        async fn returns_success_when_creating_alliance_with_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let endpoints = test.with_alliance_endpoint(1, Some(1), 1);

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.create_alliance(alliance_id).await;

            assert!(result.is_ok());
            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when fetching & creating an alliance without a faction ID
        #[tokio::test]
        async fn returns_success_when_creating_alliance_without_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let endpoints = test.with_alliance_endpoint(1, None, 1);

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.create_alliance(alliance_id).await;

            assert!(result.is_ok());
            // Assert 1 request was made to mock alliance endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error when ESI endpoint for alliance is unavailable
        #[tokio::test]
        async fn returns_error_when_endpoint_is_unavailable() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.create_alliance(alliance_id).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error when trying to create alliance that already exists
        #[tokio::test]
        async fn returns_error_when_creating_duplicate_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_model = test.insert_mock_alliance(1, None).await?;
            let endpoints = test.with_alliance_endpoint(1, None, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .create_alliance(alliance_model.alliance_id)
                .await;

            assert!(matches!(result, Err(Error::DbErr(_))));
            // Assert 1 request was made to mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }
    }

    mod get_or_create_alliance {
        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::eve::alliance::AllianceService};

        // Expect Ok with found when alliance exists in database
        #[tokio::test]
        async fn returns_success_when_alliance_exists() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_model = test.insert_mock_alliance(1, None).await?;

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .get_or_create_alliance(alliance_model.alliance_id)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        // Expect Ok when creating new alliance which does not exist in database
        #[tokio::test]
        async fn returns_success_when_creating_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_id = 1;
            let endpoints = test.with_alliance_endpoint(alliance_id, None, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_or_create_alliance(alliance_id).await;

            assert!(result.is_ok());
            // Assert 1 request was made to alliance endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error due to required tables not being created
        #[tokio::test]
        async fn returns_error_due_to_missing_required_tables() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_or_create_alliance(alliance_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        // Expect Error when required ESI endpoint is unavailable
        #[tokio::test]
        async fn returns_error_when_endpoint_is_unavailable() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_or_create_alliance(alliance_id).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }
    }
}
