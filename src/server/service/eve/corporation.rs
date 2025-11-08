use eve_esi::model::corporation::Corporation;
use futures::future::join_all;
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

    /// Fetches a list of corporations from ESI using their corporation IDs
    /// Makes concurrent requests in batches of up to 10 at a time
    pub async fn get_many_corporations(
        &self,
        corporation_ids: Vec<i64>,
    ) -> Result<Vec<(i64, Corporation)>, Error> {
        const BATCH_SIZE: usize = 10;
        let mut all_corporations = Vec::new();

        // Process corporation IDs in chunks of BATCH_SIZE
        for chunk in corporation_ids.chunks(BATCH_SIZE) {
            // Create futures for all requests in this batch
            let futures: Vec<_> = chunk
                .iter()
                .map(|&corporation_id| async move {
                    let corporation = self
                        .esi_client
                        .corporation()
                        .get_corporation_information(corporation_id)
                        .await?;
                    Ok::<(i64, Corporation), Error>((corporation_id, corporation))
                })
                .collect();

            // Execute all futures in this batch concurrently
            let results = join_all(futures).await;

            // Collect results, propagating any errors
            for result in results {
                all_corporations.push(result?);
            }
        }

        Ok(all_corporations)
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

            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(1, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());
            corporation_endpoint.assert();

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

            let alliance_id = 1;
            let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, None);
            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(1, Some(alliance_id), None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());
            alliance_endpoint.assert();
            corporation_endpoint.assert();

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

            let faction_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(1, None, Some(faction_id));

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());
            faction_endpoint.assert();
            corporation_endpoint.assert();

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

            let faction_id = 1;
            let alliance_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, Some(faction_id));
            let (corporation_id, mock_corporation) =
                test.eve()
                    .with_mock_corporation(1, Some(alliance_id), Some(faction_id));

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.create_corporation(corporation_id).await;

            assert!(result.is_ok());
            faction_endpoint.assert();
            alliance_endpoint.assert();
            corporation_endpoint.assert();

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
            let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

            let (_, mock_corporation) =
                test.eve()
                    .with_mock_corporation(corporation_model.corporation_id, None, None);

            let corporation_endpoint = test.eve().with_corporation_endpoint(
                corporation_model.corporation_id,
                mock_corporation,
                1,
            );

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .create_corporation(corporation_model.corporation_id)
                .await;

            assert!(matches!(result, Err(Error::DbErr(_))));
            corporation_endpoint.assert();

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

            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(1, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_or_create_corporation(corporation_id)
                .await;

            assert!(result.is_ok());
            corporation_endpoint.assert();

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

    mod get_many_corporations {
        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::eve::corporation::CorporationService};

        /// Expect Ok when fetching multiple corporations successfully
        #[tokio::test]
        async fn fetches_multiple_corporations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoints for 3 different corporations
            let corporation_ids = vec![1, 2, 3];
            let mut endpoints = Vec::new();
            for id in &corporation_ids {
                let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
                endpoints.push(
                    test.eve()
                        .with_corporation_endpoint(corp_id, mock_corporation, 1),
                );
            }

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids.clone())
                .await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 3);

            // Verify all corporations were returned
            for (corporation_id, _corporation) in corporations.iter() {
                assert!(corporation_ids.contains(&corporation_id));
            }

            // Assert all requests were made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok with empty vec when given empty corporation IDs list
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.get_many_corporations(vec![]).await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 0);

            Ok(())
        }

        /// Expect Ok when fetching single corporation
        #[tokio::test]
        async fn fetches_single_corporation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(1, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(vec![corporation_id])
                .await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 1);
            assert_eq!(corporations[0].0, corporation_id);

            corporation_endpoint.assert();

            Ok(())
        }

        /// Expect Ok when fetching corporations with alliances
        #[tokio::test]
        async fn fetches_corporations_with_alliances() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoints for corporations with alliances
            let alliance_id_1 = 1;
            let alliance_id_2 = 2;
            let (_, mock_alliance_1) = test.eve().with_mock_alliance(alliance_id_1, None);
            let (_, mock_alliance_2) = test.eve().with_mock_alliance(alliance_id_2, None);
            let (corporation_id_1, mock_corporation_1) =
                test.eve()
                    .with_mock_corporation(1, Some(alliance_id_1), None);
            let (corporation_id_2, mock_corporation_2) =
                test.eve()
                    .with_mock_corporation(2, Some(alliance_id_2), None);

            let alliance_endpoint_1 =
                test.eve()
                    .with_alliance_endpoint(alliance_id_1, mock_alliance_1, 1);
            let alliance_endpoint_2 =
                test.eve()
                    .with_alliance_endpoint(alliance_id_2, mock_alliance_2, 1);
            let corporation_endpoint_1 =
                test.eve()
                    .with_corporation_endpoint(corporation_id_1, mock_corporation_1, 1);
            let corporation_endpoint_2 =
                test.eve()
                    .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);

            let corporation_ids = vec![corporation_id_1, corporation_id_2];
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids)
                .await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 2);

            alliance_endpoint_1.assert();
            alliance_endpoint_2.assert();
            corporation_endpoint_1.assert();
            corporation_endpoint_2.assert();

            Ok(())
        }

        /// Expect Ok when fetching corporations with factions
        #[tokio::test]
        async fn fetches_corporations_with_factions() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoints for corporations with factions
            let faction_id_1 = 1;
            let faction_id_2 = 2;
            let mock_faction_1 = test.eve().with_mock_faction(faction_id_1);
            let mock_faction_2 = test.eve().with_mock_faction(faction_id_2);
            let (corporation_id_1, mock_corporation_1) =
                test.eve()
                    .with_mock_corporation(1, None, Some(faction_id_1));
            let (corporation_id_2, mock_corporation_2) =
                test.eve()
                    .with_mock_corporation(2, None, Some(faction_id_2));

            let faction_endpoint = test
                .eve()
                .with_faction_endpoint(vec![mock_faction_1, mock_faction_2], 1);
            let corporation_endpoint_1 =
                test.eve()
                    .with_corporation_endpoint(corporation_id_1, mock_corporation_1, 1);
            let corporation_endpoint_2 =
                test.eve()
                    .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);

            let corporation_ids = vec![corporation_id_1, corporation_id_2];
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids)
                .await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 2);

            faction_endpoint.assert();
            corporation_endpoint_1.assert();
            corporation_endpoint_2.assert();

            Ok(())
        }

        /// Expect Ok when fetching corporations with both alliance and faction
        #[tokio::test]
        async fn fetches_corporations_with_alliance_and_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoint for corporation with alliance and faction
            let faction_id = 1;
            let alliance_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, Some(faction_id));
            let (corporation_id, mock_corporation) =
                test.eve()
                    .with_mock_corporation(1, Some(alliance_id), Some(faction_id));

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(vec![corporation_id])
                .await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 1);
            assert_eq!(corporations[0].1.alliance_id, Some(alliance_id));
            assert_eq!(corporations[0].1.faction_id, Some(faction_id));

            faction_endpoint.assert();
            alliance_endpoint.assert();
            corporation_endpoint.assert();

            Ok(())
        }

        /// Expect Error when ESI endpoint is unavailable for any corporation
        #[tokio::test]
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_ids = vec![1, 2, 3];
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids)
                .await;

            // Should fail on first unavailable corporation
            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error when ESI fails partway through batch
        #[tokio::test]
        async fn fails_on_partial_esi_failure() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoint for first corporation only
            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(1, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_ids = vec![1, 2, 3];
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids)
                .await;

            // Should succeed on first, fail on second (no mock)
            assert!(matches!(result, Err(Error::EsiError(_))));

            corporation_endpoint.assert();

            Ok(())
        }

        /// Expect Ok when fetching many corporations (stress test)
        #[tokio::test]
        async fn fetches_many_corporations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoints for 10 corporations
            let corporation_ids: Vec<i64> = (1..=10).collect();
            let mut endpoints = Vec::new();
            for id in &corporation_ids {
                let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
                endpoints.push(
                    test.eve()
                        .with_corporation_endpoint(corp_id, mock_corporation, 1),
                );
            }

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids.clone())
                .await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 10);

            // Verify all corporation IDs are present
            for (corporation_id, _corporation) in corporations.iter() {
                assert!(corporation_ids.contains(&corporation_id));
            }

            // Assert all requests were made
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when fetching more than 10 corporations (tests batching)
        #[tokio::test]
        async fn fetches_many_corporations_with_batching() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoints for 25 corporations to test multiple batches
            let corporation_ids: Vec<i64> = (1..=25).collect();
            let mut endpoints = Vec::new();
            for id in &corporation_ids {
                let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
                endpoints.push(
                    test.eve()
                        .with_corporation_endpoint(corp_id, mock_corporation, 1),
                );
            }

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids.clone())
                .await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 25);

            // Verify all corporation IDs are present (order may vary due to concurrency)
            let returned_ids: Vec<i64> = corporations.iter().map(|(id, _)| *id).collect();
            for id in &corporation_ids {
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
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoints for 5 corporations (within one batch)
            let corporation_ids: Vec<i64> = (1..=5).collect();
            let mut endpoints = Vec::new();
            for id in &corporation_ids {
                let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
                endpoints.push(
                    test.eve()
                        .with_corporation_endpoint(corp_id, mock_corporation, 1),
                );
            }

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids.clone())
                .await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 5);

            // Verify all corporation IDs are present
            let returned_ids: Vec<i64> = corporations.iter().map(|(id, _)| *id).collect();
            for id in &corporation_ids {
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
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoints for only some corporations in the batch
            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(1, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_ids = vec![1, 2, 3, 4, 5];
            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids)
                .await;

            // Should fail when any request in the batch fails
            assert!(matches!(result, Err(Error::EsiError(_))));

            corporation_endpoint.assert();

            Ok(())
        }

        /// Expect correct batching behavior with exactly 10 items
        #[tokio::test]
        async fn handles_exact_batch_size() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoints for exactly 10 corporations (one full batch)
            let corporation_ids: Vec<i64> = (1..=10).collect();
            let mut endpoints = Vec::new();
            for id in &corporation_ids {
                let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
                endpoints.push(
                    test.eve()
                        .with_corporation_endpoint(corp_id, mock_corporation, 1),
                );
            }

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids.clone())
                .await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 10);

            // Verify all corporation IDs are present
            let returned_ids: Vec<i64> = corporations.iter().map(|(id, _)| *id).collect();
            for id in &corporation_ids {
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
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Setup mock endpoints for 11 corporations (one full batch + one item)
            let corporation_ids: Vec<i64> = (1..=11).collect();
            let mut endpoints = Vec::new();
            for id in &corporation_ids {
                let (corp_id, mock_corporation) = test.eve().with_mock_corporation(*id, None, None);
                endpoints.push(
                    test.eve()
                        .with_corporation_endpoint(corp_id, mock_corporation, 1),
                );
            }

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .get_many_corporations(corporation_ids.clone())
                .await;

            assert!(result.is_ok());
            let corporations = result.unwrap();
            assert_eq!(corporations.len(), 11);

            // Verify all corporation IDs are present
            let returned_ids: Vec<i64> = corporations.iter().map(|(id, _)| *id).collect();
            for id in &corporation_ids {
                assert!(returned_ids.contains(id));
            }

            // Assert all requests were made
            for endpoint in endpoints {
                endpoint.assert();
            }

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

            let faction_id = 1;
            let alliance_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, Some(faction_id));
            let (corporation_id, mock_corporation) =
                test.eve()
                    .with_mock_corporation(1, Some(alliance_id), Some(faction_id));

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.upsert_corporation(corporation_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.corporation_id, corporation_id);
            assert!(created.alliance_id.is_some());
            assert!(created.faction_id.is_some());

            faction_endpoint.assert();
            alliance_endpoint.assert();
            corporation_endpoint.assert();

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

            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(1, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.upsert_corporation(corporation_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.corporation_id, corporation_id);
            assert_eq!(created.alliance_id, None);
            assert_eq!(created.faction_id, None);

            corporation_endpoint.assert();

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

            let (_, mock_corporation) =
                test.eve()
                    .with_mock_corporation(corporation_model.corporation_id, None, None);

            let corporation_endpoint = test.eve().with_corporation_endpoint(
                corporation_model.corporation_id,
                mock_corporation,
                1,
            );

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

            corporation_endpoint.assert();

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
            let alliance_id_2 = 2;
            let (_, mock_alliance_2) = test.eve().with_mock_alliance(alliance_id_2, None);
            let (_, mock_corporation) = test.eve().with_mock_corporation(
                corporation_model.corporation_id,
                Some(alliance_id_2),
                None,
            );

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id_2, mock_alliance_2, 1);
            let corporation_endpoint = test.eve().with_corporation_endpoint(
                corporation_model.corporation_id,
                mock_corporation,
                1,
            );

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_ne!(upserted.alliance_id, corporation_model.alliance_id);

            alliance_endpoint.assert();
            corporation_endpoint.assert();

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
            let faction_id_2 = 2;
            let mock_faction_2 = test.eve().with_mock_faction(faction_id_2);
            let (_, mock_corporation) = test.eve().with_mock_corporation(
                corporation_model.corporation_id,
                None,
                Some(faction_id_2),
            );

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction_2], 1);
            let corporation_endpoint = test.eve().with_corporation_endpoint(
                corporation_model.corporation_id,
                mock_corporation,
                1,
            );

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_ne!(upserted.faction_id, corporation_model.faction_id);

            faction_endpoint.assert();
            corporation_endpoint.assert();

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
            let (_, mock_corporation) =
                test.eve()
                    .with_mock_corporation(corporation_model.corporation_id, None, None);

            let corporation_endpoint = test.eve().with_corporation_endpoint(
                corporation_model.corporation_id,
                mock_corporation,
                1,
            );

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_eq!(upserted.alliance_id, None);

            corporation_endpoint.assert();

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
            let (_, mock_corporation) =
                test.eve()
                    .with_mock_corporation(corporation_model.corporation_id, None, None);

            let corporation_endpoint = test.eve().with_corporation_endpoint(
                corporation_model.corporation_id,
                mock_corporation,
                1,
            );

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_eq!(upserted.faction_id, None);

            corporation_endpoint.assert();

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
            let alliance_id = 1;
            let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, None);
            let (_, mock_corporation) = test.eve().with_mock_corporation(
                corporation_model.corporation_id,
                Some(alliance_id),
                None,
            );

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);
            let corporation_endpoint = test.eve().with_corporation_endpoint(
                corporation_model.corporation_id,
                mock_corporation,
                1,
            );

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert!(upserted.alliance_id.is_some());

            alliance_endpoint.assert();
            corporation_endpoint.assert();

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
            let faction_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (_, mock_corporation) = test.eve().with_mock_corporation(
                corporation_model.corporation_id,
                None,
                Some(faction_id),
            );

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let corporation_endpoint = test.eve().with_corporation_endpoint(
                corporation_model.corporation_id,
                mock_corporation,
                1,
            );

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service
                .upsert_corporation(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert!(upserted.faction_id.is_some());

            faction_endpoint.assert();
            corporation_endpoint.assert();

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

            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(1, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let corporation_service =
                CorporationService::new(&test.state.db, &test.state.esi_client);
            let result = corporation_service.upsert_corporation(corporation_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            corporation_endpoint.assert();

            Ok(())
        }
    }
}
