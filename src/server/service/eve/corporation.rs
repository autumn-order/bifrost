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

    /// Fetches a corporation from EVE Online's ESI and updates or creates a database entry
    pub async fn upsert_corporation(
        &self,
        corporation_id: i64,
    ) -> Result<entity::eve_corporation::Model, Error> {
        let corporation_repo = CorporationRepository::new(&self.db);
        let alliance_service = AllianceService::new(&self.db, &self.esi_client);
        let faction_service = FactionService::new(&self.db, &self.esi_client);

        // Get corporation information from ESI
        let corporation = self
            .esi_client
            .corporation()
            .get_corporation_information(corporation_id)
            .await?;

        // Ensure alliance exists in database or create it if applicable to prevent foreign key error
        let alliance_id = match corporation.alliance_id {
            Some(alliance_id) => Some(
                alliance_service
                    .get_or_create_alliance(alliance_id)
                    .await?
                    .id,
            ),
            None => None,
        };

        // Ensure faction exists in database or create it if applicable to prevent foreign key error
        let faction_id = match corporation.faction_id {
            Some(faction_id) => Some(faction_service.get_or_update_factions(faction_id).await?.id),
            None => None,
        };

        // Update or create corporation in database
        let corporation = corporation_repo
            .upsert(corporation_id, corporation, alliance_id, faction_id)
            .await?;

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
        async fn creates_corporation_without_alliance_or_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_id = 1;
            let endpoints = test
                .eve()
                .with_corporation_endpoint(corporation_id, None, None, 1);

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
        async fn creates_corporation_with_alliance() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_id = 1;
            let endpoints = test
                .eve()
                .with_corporation_endpoint(corporation_id, Some(1), None, 1);

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
        async fn creates_corporation_with_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_id = 1;
            let endpoints = test
                .eve()
                .with_corporation_endpoint(corporation_id, None, Some(1), 1);

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
        async fn creates_corporation_with_alliance_and_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_id = 1;
            let endpoints =
                test.eve()
                    .with_corporation_endpoint(corporation_id, Some(1), Some(1), 1);

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
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
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
        async fn fails_for_duplicate_corporation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;
            let corporation_id = 1;
            let _ = test
                .eve()
                .insert_mock_corporation(corporation_id, None, None)
                .await?;
            let endpoints = test
                .eve()
                .with_corporation_endpoint(corporation_id, None, None, 1);

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
        async fn finds_existing_corporation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

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
        async fn creates_corporation_when_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_id = 1;
            let endpoints = test
                .eve()
                .with_corporation_endpoint(corporation_id, None, None, 1);

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
        async fn fails_when_tables_missing() -> Result<(), TestError> {
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
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
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

    mod upsert_corporation {
        use chrono::{Duration, Utc};
        use sea_orm::{ActiveValue, EntityTrait, IntoActiveModel};

        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::eve::corporation::CorporationService};

        /// Expect Ok when upserting a new corporation with alliance and faction
        #[tokio::test]
        async fn creates_new_corporation_with_alliance_and_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let endpoints = test.eve().with_corporation_endpoint(1, Some(1), Some(1), 1);

            let corporation_id = 1;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.upsert_corporation(corporation_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.corporation_id, corporation_id);
            assert!(created.alliance_id.is_some());
            assert!(created.faction_id.is_some());

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

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
            let endpoints = test.eve().with_corporation_endpoint(1, None, None, 1);

            let corporation_id = 1;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.upsert_corporation(corporation_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.corporation_id, corporation_id);
            assert_eq!(created.alliance_id, None);
            assert_eq!(created.faction_id, None);

            // Assert 1 request was made to mock corporation endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

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
            let endpoints = test.eve().with_corporation_endpoint(1, None, None, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            // Verify the ID remains the same (it's an update, not a new insert)
            assert_eq!(upserted.id, corporation_model.id);
            assert_eq!(upserted.corporation_id, corporation_model.corporation_id);

            // Assert 1 request was made to mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

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
            let corporation_model = test
                .eve()
                .insert_mock_corporation(1, Some(alliance_model1.alliance_id), None)
                .await?;

            // Mock endpoint returns corporation with different alliance
            let endpoints = test.eve().with_corporation_endpoint(1, Some(2), None, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_ne!(upserted.alliance_id, corporation_model.alliance_id);

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

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

            // Set faction last updated before today's faction update window to allow for updating
            // the faction from ESI
            let mut faction_model_am = entity::prelude::EveFaction::find_by_id(faction_model1.id)
                .one(&test.state.db)
                .await?
                .unwrap()
                .into_active_model();

            faction_model_am.updated_at =
                ActiveValue::Set((Utc::now() - Duration::hours(24)).naive_utc());

            entity::prelude::EveFaction::update(faction_model_am)
                .exec(&test.state.db)
                .await?;

            let corporation_model = test
                .eve()
                .insert_mock_corporation(1, None, Some(faction_model1.faction_id))
                .await?;

            // Mock endpoint returns corporation with different faction
            let endpoints = test.eve().with_corporation_endpoint(1, None, Some(2), 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_ne!(upserted.faction_id, corporation_model.faction_id);

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

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

            // Mock endpoint returns corporation without alliance
            let endpoints = test.eve().with_corporation_endpoint(1, None, None, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_eq!(upserted.alliance_id, None);

            // Assert 1 request was made to mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

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

            // Mock endpoint returns corporation without faction
            let endpoints = test.eve().with_corporation_endpoint(1, None, None, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_eq!(upserted.faction_id, None);

            // Assert 1 request was made to mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when upserting adds alliance relationship
        #[tokio::test]
        async fn adds_alliance_relationship_on_upsert() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

            assert_eq!(corporation_model.alliance_id, None);

            // Mock endpoint returns corporation with alliance
            let endpoints = test.eve().with_corporation_endpoint(1, Some(1), None, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert!(upserted.alliance_id.is_some());

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when upserting adds faction relationship
        #[tokio::test]
        async fn adds_faction_relationship_on_upsert() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

            assert_eq!(corporation_model.faction_id, None);

            // Mock endpoint returns corporation with faction
            let endpoints = test.eve().with_corporation_endpoint(1, None, Some(1), 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert!(upserted.faction_id.is_some());

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error when ESI endpoint for corporation is unavailable
        #[tokio::test]
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_id = 1;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.upsert_corporation(corporation_id).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error due to required tables not being created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!()?;
            let endpoints = test.eve().with_corporation_endpoint(1, None, None, 1);

            let corporation_id = 1;
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.upsert_corporation(corporation_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            // Assert 1 request was made to mock endpoint before DB error
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }
    }
}
