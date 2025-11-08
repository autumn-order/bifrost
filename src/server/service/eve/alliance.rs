use eve_esi::model::alliance::Alliance;
use futures::future::join_all;
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
    /// Makes concurrent requests in batches of up to 10 at a time
    pub async fn get_many_alliances(
        &self,
        alliance_ids: Vec<i64>,
    ) -> Result<Vec<(i64, Alliance)>, Error> {
        const BATCH_SIZE: usize = 10;
        let mut all_alliances = Vec::new();

        for chunk in alliance_ids.chunks(BATCH_SIZE) {
            let futures: Vec<_> = chunk
                .iter()
                .map(|&alliance_id| async move {
                    let alliance = self
                        .esi_client
                        .alliance()
                        .get_alliance_information(alliance_id)
                        .await?;
                    Ok::<(i64, Alliance), Error>((alliance_id, alliance))
                })
                .collect();

            let results = join_all(futures).await;

            for result in results {
                all_alliances.push(result?);
            }
        }

        Ok(all_alliances)
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

            let faction_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, Some(faction_id));

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.create_alliance(alliance_id).await;

            assert!(result.is_ok());
            faction_endpoint.assert();
            alliance_endpoint.assert();

            Ok(())
        }

        /// Expect Ok when fetching & creating an alliance without a faction ID
        #[tokio::test]
        async fn creates_alliance_without_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.create_alliance(alliance_id).await;

            assert!(result.is_ok());
            alliance_endpoint.assert();

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

            let (_, mock_alliance) = test
                .eve()
                .with_mock_alliance(alliance_model.alliance_id, None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_model.alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .create_alliance(alliance_model.alliance_id)
                .await;

            assert!(matches!(result, Err(Error::DbErr(_))));
            alliance_endpoint.assert();

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

            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_or_create_alliance(alliance_id).await;

            assert!(result.is_ok());
            alliance_endpoint.assert();

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
                let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
                endpoints.push(
                    test.eve()
                        .with_alliance_endpoint(alliance_id, mock_alliance, 1),
                );
            }

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .get_many_alliances(alliance_ids.clone())
                .await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 3);

            // Verify all alliance IDs are present (order may vary due to concurrency)
            let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
            for id in &alliance_ids {
                assert!(returned_ids.contains(id));
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

            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_many_alliances(vec![alliance_id]).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 1);
            assert_eq!(alliances[0].0, alliance_id);

            alliance_endpoint.assert();

            Ok(())
        }

        /// Expect Ok when fetching alliances with factions
        #[tokio::test]
        async fn fetches_alliances_with_factions() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            // Setup mock endpoints for alliances with factions
            let faction_id_1 = 1;
            let faction_id_2 = 2;
            let mock_faction_1 = test.eve().with_mock_faction(faction_id_1);
            let mock_faction_2 = test.eve().with_mock_faction(faction_id_2);

            let (alliance_id_1, mock_alliance_1) =
                test.eve().with_mock_alliance(1, Some(faction_id_1));
            let (alliance_id_2, mock_alliance_2) =
                test.eve().with_mock_alliance(2, Some(faction_id_2));

            let faction_endpoint = test
                .eve()
                .with_faction_endpoint(vec![mock_faction_1, mock_faction_2], 1);
            let alliance_endpoint_1 =
                test.eve()
                    .with_alliance_endpoint(alliance_id_1, mock_alliance_1, 1);
            let alliance_endpoint_2 =
                test.eve()
                    .with_alliance_endpoint(alliance_id_2, mock_alliance_2, 1);

            let alliance_ids = vec![alliance_id_1, alliance_id_2];
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_many_alliances(alliance_ids).await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 2);

            faction_endpoint.assert();
            alliance_endpoint_1.assert();
            alliance_endpoint_2.assert();

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
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let alliance_ids = vec![1, 2, 3];
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_many_alliances(alliance_ids).await;

            // Should succeed on first, fail on second (no mock)
            assert!(matches!(result, Err(Error::EsiError(_))));

            alliance_endpoint.assert();

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
                let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
                endpoints.push(
                    test.eve()
                        .with_alliance_endpoint(alliance_id, mock_alliance, 1),
                );
            }

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .get_many_alliances(alliance_ids.clone())
                .await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 10);

            // Verify all alliance IDs are present (order may vary due to concurrency)
            let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
            for id in &alliance_ids {
                assert!(returned_ids.contains(id));
            }

            // Assert all requests were made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when fetching more than 10 alliances (tests batching)
        #[tokio::test]
        async fn fetches_many_alliances_with_batching() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            // Setup mock endpoints for 25 alliances to test multiple batches
            let alliance_ids: Vec<i64> = (1..=25).collect();
            let mut endpoints = Vec::new();
            for id in &alliance_ids {
                let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
                endpoints.push(
                    test.eve()
                        .with_alliance_endpoint(alliance_id, mock_alliance, 1),
                );
            }

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .get_many_alliances(alliance_ids.clone())
                .await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 25);

            // Verify all alliance IDs are present (order may vary due to concurrency)
            let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
            for id in &alliance_ids {
                assert!(returned_ids.contains(id));
            }

            // Assert all requests were made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when verifying concurrent execution within a batch
        #[tokio::test]
        async fn executes_requests_concurrently() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            // Setup mock endpoints for 5 alliances (within one batch)
            let alliance_ids: Vec<i64> = (1..=5).collect();
            let mut endpoints = Vec::new();
            for id in &alliance_ids {
                let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
                endpoints.push(
                    test.eve()
                        .with_alliance_endpoint(alliance_id, mock_alliance, 1),
                );
            }

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .get_many_alliances(alliance_ids.clone())
                .await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 5);

            // Verify all alliance IDs are present
            let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
            for id in &alliance_ids {
                assert!(returned_ids.contains(id));
            }

            // Assert all requests were made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error when ESI fails in middle of concurrent batch
        #[tokio::test]
        async fn fails_on_concurrent_batch_error() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            // Setup mock endpoints for only some alliances in the batch
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let alliance_ids = vec![1, 2, 3, 4, 5];
            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.get_many_alliances(alliance_ids).await;

            // Should fail when any request in the batch fails
            assert!(matches!(result, Err(Error::EsiError(_))));

            alliance_endpoint.assert();

            Ok(())
        }

        /// Expect correct batching behavior with exactly 10 items
        #[tokio::test]
        async fn handles_exact_batch_size() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            // Setup mock endpoints for exactly 10 alliances (one full batch)
            let alliance_ids: Vec<i64> = (1..=10).collect();
            let mut endpoints = Vec::new();
            for id in &alliance_ids {
                let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
                endpoints.push(
                    test.eve()
                        .with_alliance_endpoint(alliance_id, mock_alliance, 1),
                );
            }

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .get_many_alliances(alliance_ids.clone())
                .await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 10);

            // Verify all alliance IDs are present
            let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
            for id in &alliance_ids {
                assert!(returned_ids.contains(id));
            }

            // Assert all requests were made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect correct batching behavior with 11 items (tests partial second batch)
        #[tokio::test]
        async fn handles_batch_size_plus_one() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            // Setup mock endpoints for 11 alliances (one full batch + one item)
            let alliance_ids: Vec<i64> = (1..=11).collect();
            let mut endpoints = Vec::new();
            for id in &alliance_ids {
                let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(*id, None);
                endpoints.push(
                    test.eve()
                        .with_alliance_endpoint(alliance_id, mock_alliance, 1),
                );
            }

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .get_many_alliances(alliance_ids.clone())
                .await;

            assert!(result.is_ok());
            let alliances = result.unwrap();
            assert_eq!(alliances.len(), 11);

            // Verify all alliance IDs are present
            let returned_ids: Vec<i64> = alliances.iter().map(|(id, _)| *id).collect();
            for id in &alliance_ids {
                assert!(returned_ids.contains(id));
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

            let faction_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, Some(faction_id));

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.upsert_alliance(alliance_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.alliance_id, alliance_id);
            assert!(created.faction_id.is_some());

            faction_endpoint.assert();
            alliance_endpoint.assert();

            Ok(())
        }

        /// Expect Ok when upserting a new alliance without a faction ID
        #[tokio::test]
        async fn creates_new_alliance_without_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.upsert_alliance(alliance_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.alliance_id, alliance_id);
            assert_eq!(created.faction_id, None);

            alliance_endpoint.assert();

            Ok(())
        }

        /// Expect Ok when upserting an existing alliance and verify it updates
        #[tokio::test]
        async fn updates_existing_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;

            let (_, mock_alliance) = test
                .eve()
                .with_mock_alliance(alliance_model.alliance_id, None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_model.alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .upsert_alliance(alliance_model.alliance_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            // Verify the ID remains the same (it's an update, not a new insert)
            assert_eq!(upserted.id, alliance_model.id);
            assert_eq!(upserted.alliance_id, alliance_model.alliance_id);

            alliance_endpoint.assert();

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
            let faction_id_2 = 2;
            let mock_faction_2 = test.eve().with_mock_faction(faction_id_2);
            let (_, mock_alliance) = test
                .eve()
                .with_mock_alliance(alliance_model.alliance_id, Some(faction_id_2));

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction_2], 1);
            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_model.alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .upsert_alliance(alliance_model.alliance_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, alliance_model.id);
            assert_ne!(upserted.faction_id, alliance_model.faction_id);

            faction_endpoint.assert();
            alliance_endpoint.assert();

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
            let (_, mock_alliance) = test
                .eve()
                .with_mock_alliance(alliance_model.alliance_id, None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_model.alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .upsert_alliance(alliance_model.alliance_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, alliance_model.id);
            assert_eq!(upserted.faction_id, None);

            alliance_endpoint.assert();

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
            let faction_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (_, mock_alliance) = test
                .eve()
                .with_mock_alliance(alliance_model.alliance_id, Some(faction_id));

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_model.alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service
                .upsert_alliance(alliance_model.alliance_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, alliance_model.id);
            assert!(upserted.faction_id.is_some());

            faction_endpoint.assert();
            alliance_endpoint.assert();

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

            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let alliance_service = AllianceService::new(&test.state.db, &test.state.esi_client);
            let result = alliance_service.upsert_alliance(alliance_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            alliance_endpoint.assert();

            Ok(())
        }
    }
}
