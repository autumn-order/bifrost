use eve_esi::model::alliance::Alliance;
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

    /// Fetches a list of alliances from ESI using their alliance IDs
    pub async fn get_many_alliances(
        &self,
        alliance_ids: Vec<i64>,
    ) -> Result<Vec<(i64, Alliance)>, Error> {
        let mut alliances = Vec::new();

        for alliance_id in alliance_ids {
            let alliance = self
                .esi_client
                .alliance()
                .get_alliance_information(alliance_id)
                .await?;

            alliances.push((alliance_id, alliance))
        }

        Ok(alliances)
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

    /// Updates or creates an entry for provided alliance ID
    pub async fn upsert_alliance(
        &self,
        alliance_id: i64,
    ) -> Result<entity::eve_alliance::Model, Error> {
        let alliance_repo = AllianceRepository::new(&self.db);
        let faction_service = FactionService::new(&self.db, &self.esi_client);

        // Get alliance information from ESI
        let alliance = self
            .esi_client
            .alliance()
            .get_alliance_information(alliance_id)
            .await?;

        // Ensure faction exists in database or create it if applicable to prevent foreign key error
        let faction_id = match alliance.faction_id {
            Some(faction_id) => Some(faction_service.get_or_update_factions(faction_id).await?.id),
            None => None,
        };

        // Update or create alliance in database
        let alliance = alliance_repo
            .upsert(alliance_id, alliance, faction_id)
            .await?;

        Ok(alliance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bifrost_test_utils::prelude::*;

    mod create_alliance {
        use super::*;

        /// Expect Ok when fetching & creating an alliance with a faction ID
        #[tokio::test]
        async fn creates_alliance_with_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let endpoints = test.eve().with_alliance_endpoint(1, Some(1), 1);

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
        async fn creates_alliance_without_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let endpoints = test.eve().with_alliance_endpoint(1, None, 1);

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
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
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
        async fn fails_for_duplicate_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
            let endpoints = test.eve().with_alliance_endpoint(1, None, 1);

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
        use super::*;

        // Expect Ok with found when alliance exists in database
        #[tokio::test]
        async fn finds_existing_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .get_or_create_alliance(alliance_model.alliance_id)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        // Expect Ok when creating new alliance which does not exist in database
        #[tokio::test]
        async fn creates_alliance_when_missing() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_id = 1;
            let endpoints = test.eve().with_alliance_endpoint(alliance_id, None, 1);

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
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_or_create_alliance(alliance_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        // Expect Error when required ESI endpoint is unavailable
        #[tokio::test]
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_or_create_alliance(alliance_id).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }
    }

    mod get_many_alliances {
        use super::*;

        /// Expect Ok when fetching multiple alliances successfully
        #[tokio::test]
        async fn fetches_multiple_alliances() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            // Setup mock endpoints for 3 different alliances
            let alliance_ids = vec![1, 2, 3];
            let mut endpoints = Vec::new();
            for id in &alliance_ids {
                endpoints.extend(test.eve().with_alliance_endpoint(*id, None, 1));
            }

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .get_many_alliances(alliance_ids.clone())
                .await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 3);

            // Verify all alliance IDs are present
            for (idx, (alliance_id, _)) in alliances.iter().enumerate() {
                assert_eq!(alliance_id, &alliance_ids[idx]);
            }

            // Assert all requests were made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok with empty vec when given empty alliance IDs list
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_many_alliances(vec![]).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 0);

            Ok(())
        }

        /// Expect Ok when fetching single alliance
        #[tokio::test]
        async fn fetches_single_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let alliance_id = 1;
            let endpoints = test.eve().with_alliance_endpoint(alliance_id, None, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_many_alliances(vec![alliance_id]).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 1);
            assert_eq!(alliances[0].0, alliance_id);

            // Assert request was made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when fetching alliances with factions
        // Need to implement test endpoint builder to test this properly, unfortunately this will
        // error due to the faction endpoint created by `with_alliance_endpoint` not getting any
        // requests despite expecting 1.
        //
        // More fine-grained control over endpoints is necessary for this test.
        #[tokio::test]
        #[ignore]
        async fn fetches_alliances_with_factions() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            // Setup mock endpoints for alliances with factions
            let alliance_ids = vec![1, 2];
            let mut endpoints = Vec::new();
            endpoints.extend(test.eve().with_alliance_endpoint(1, Some(1), 1));
            endpoints.extend(test.eve().with_alliance_endpoint(2, Some(2), 1));

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_many_alliances(alliance_ids).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 2);
            assert_eq!(alliances[0].1.faction_id, Some(1));
            assert_eq!(alliances[1].1.faction_id, Some(1));

            // Assert all requests were made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error when ESI endpoint is unavailable for any alliance
        #[tokio::test]
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let alliance_ids = vec![1, 2, 3];
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_many_alliances(alliance_ids).await;

            // Should fail on first unavailable alliance
            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error when ESI fails partway through batch
        #[tokio::test]
        async fn fails_on_partial_esi_failure() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            // Setup mock endpoint for first alliance only
            let endpoints = test.eve().with_alliance_endpoint(1, None, 1);

            let alliance_ids = vec![1, 2, 3];
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_many_alliances(alliance_ids).await;

            // Should succeed on first, fail on second (no mock)
            assert!(matches!(result, Err(Error::EsiError(_))));

            // Assert first request was made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when fetching many alliances (stress test)
        #[tokio::test]
        async fn fetches_many_alliances() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            // Setup mock endpoints for 10 alliances
            let alliance_ids: Vec<i64> = (1..=10).collect();
            let mut endpoints = Vec::new();
            for id in &alliance_ids {
                endpoints.extend(test.eve().with_alliance_endpoint(*id, None, 1));
            }

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .get_many_alliances(alliance_ids.clone())
                .await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 10);

            // Verify all alliance IDs are present in order
            for (idx, (alliance_id, _)) in alliances.iter().enumerate() {
                assert_eq!(alliance_id, &alliance_ids[idx]);
            }

            // Assert all requests were made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }
    }

    mod upsert_alliance {
        use chrono::{Duration, Utc};
        use sea_orm::{ActiveValue, EntityTrait, IntoActiveModel};

        use super::*;

        /// Expect Ok when upserting a new alliance with a faction ID
        #[tokio::test]
        async fn creates_new_alliance_with_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let endpoints = test.eve().with_alliance_endpoint(1, Some(1), 1);

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.upsert_alliance(alliance_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.alliance_id, alliance_id);
            assert!(created.faction_id.is_some());

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when upserting a new alliance without a faction ID
        #[tokio::test]
        async fn creates_new_alliance_without_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let endpoints = test.eve().with_alliance_endpoint(1, None, 1);

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.upsert_alliance(alliance_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.alliance_id, alliance_id);
            assert_eq!(created.faction_id, None);

            // Assert 1 request was made to mock alliance endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when upserting an existing alliance and verify it updates
        #[tokio::test]
        async fn updates_existing_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
            let endpoints = test.eve().with_alliance_endpoint(1, None, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .upsert_alliance(alliance_model.alliance_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            // Verify the ID remains the same (it's an update, not a new insert)
            assert_eq!(upserted.id, alliance_model.id);
            assert_eq!(upserted.alliance_id, alliance_model.alliance_id);

            // Assert 1 request was made to mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when upserting an existing alliance with a new faction ID
        #[tokio::test]
        async fn updates_alliance_faction_relationship() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
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

            let alliance_model = test
                .eve()
                .insert_mock_alliance(1, Some(faction_model1.faction_id))
                .await?;

            // Mock endpoint returns alliance with different faction
            let endpoints = test.eve().with_alliance_endpoint(1, Some(2), 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .upsert_alliance(alliance_model.alliance_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, alliance_model.id);
            assert_ne!(upserted.faction_id, alliance_model.faction_id);

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when upserting removes faction relationship
        #[tokio::test]
        async fn removes_faction_relationship_on_upsert() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let alliance_model = test
                .eve()
                .insert_mock_alliance(1, Some(faction_model.faction_id))
                .await?;

            assert!(alliance_model.faction_id.is_some());

            // Mock endpoint returns alliance without faction
            let endpoints = test.eve().with_alliance_endpoint(1, None, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .upsert_alliance(alliance_model.alliance_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, alliance_model.id);
            assert_eq!(upserted.faction_id, None);

            // Assert 1 request was made to mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when upserting adds faction relationship
        #[tokio::test]
        async fn adds_faction_relationship_on_upsert() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;

            assert_eq!(alliance_model.faction_id, None);

            // Mock endpoint returns alliance with faction
            let endpoints = test.eve().with_alliance_endpoint(1, Some(1), 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .upsert_alliance(alliance_model.alliance_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, alliance_model.id);
            assert!(upserted.faction_id.is_some());

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error when ESI endpoint for alliance is unavailable
        #[tokio::test]
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.upsert_alliance(alliance_id).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error due to required tables not being created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!()?;
            let endpoints = test.eve().with_alliance_endpoint(1, None, 1);

            let alliance_id = 1;
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.upsert_alliance(alliance_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            // Assert 1 request was made to mock endpoint before DB error
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }
    }
}
