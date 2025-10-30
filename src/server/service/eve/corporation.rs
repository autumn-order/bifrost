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

    /// Get corporation from database or create an entry for it from ESI
    pub async fn get_or_create_corporation(
        &self,
        corporation_id: i64,
    ) -> Result<entity::eve_corporation::Model, Error> {
        let corporation_repo = CorporationRepository::new(&self.db);

        if let Some(corporation) = corporation_repo
            .get_by_corporation_id(corporation_id)
            .await?
        {
            return Ok(corporation);
        }

        let corporation = self.create_corporation(corporation_id).await?;

        Ok(corporation)
    }
}

#[cfg(test)]
mod tests {

    mod create_corporation {
        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::eve::corporation::CorporationService};

        /// Expect Ok when creating corporation without alliance or faction
        #[tokio::test]
        async fn create_corporation_ok_no_alliance_or_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_id = 1;
            let endpoints = test.with_corporation_endpoint(corporation_id, None, None, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());
            // Assert 1 request was made to mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when creating corporation with alliance
        #[tokio::test]
        async fn create_corporation_ok_with_alliance() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_id = 1;
            let endpoints = test.with_corporation_endpoint(corporation_id, Some(1), None, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());
            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when creating corporation with faction
        #[tokio::test]
        async fn create_corporation_ok_with_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_id = 1;
            let endpoints = test.with_corporation_endpoint(corporation_id, None, Some(1), 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when creating corporation with alliance and faction
        #[tokio::test]
        async fn create_corporation_ok_with_alliance_and_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_id = 1;
            let endpoints = test.with_corporation_endpoint(corporation_id, Some(1), Some(1), 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());
            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error when ESI endpoint is unavailable
        #[tokio::test]
        async fn create_corporation_err_esi() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_id = 1;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error when database table are not created
        #[tokio::test]
        async fn create_corporation_err_duplicate_corporation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;
            let corporation_id = 1;
            let _ = test
                .insert_mock_corporation(corporation_id, None, None)
                .await?;
            let endpoints = test.with_corporation_endpoint(corporation_id, None, None, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));
            // Assert 1 request was made to mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }
    }

    mod get_or_create_corporation {
        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::eve::corporation::CorporationService};

        // Expect Ok when corporation is found already present in table
        #[tokio::test]
        async fn get_or_create_corporation_ok_found() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_model = test.insert_mock_corporation(1, None, None).await?;

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_or_create_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        // Expect Ok when creating a new corporation when not found in table
        #[tokio::test]
        async fn get_or_create_corporation_ok_creates_if_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_id = 1;
            let endpoints = test.with_corporation_endpoint(corporation_id, None, None, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_or_create_corporation(corporation_id)
                .await;

            assert!(result.is_ok());
            // Assert 1 request was made to corporation endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error when trying to access database table that hasn't been created
        #[tokio::test]
        async fn get_or_create_corporation_err_missing_tables() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let corporation_id = 1;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_or_create_corporation(corporation_id)
                .await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        /// Expect Error when required ESI endpoint is unavailable
        #[tokio::test]
        async fn get_or_create_corporation_err_esi() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_id = 1;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_or_create_corporation(corporation_id)
                .await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }
    }
}
