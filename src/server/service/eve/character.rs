use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::character::CharacterRepository,
    error::Error,
    service::eve::{corporation::CorporationService, faction::FactionService},
};

pub struct CharacterService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CharacterService<'a> {
    /// Creates a new instance of [`CharacterService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Fetches a character from EVE Online's ESI and creates a database entry
    pub async fn create_character(
        &self,
        character_id: i64,
    ) -> Result<entity::eve_character::Model, Error> {
        let character_repo = CharacterRepository::new(&self.db);
        let corporation_service = CorporationService::new(&self.db, &self.esi_client);
        let faction_service = FactionService::new(&self.db, &self.esi_client);

        let character = self
            .esi_client
            .character()
            .get_character_public_information(character_id)
            .await?;

        let corporation_id = corporation_service
            .get_or_create_corporation(character.corporation_id)
            .await?
            .id;

        let faction_id = match character.faction_id {
            Some(id) => Some(faction_service.get_or_update_factions(id).await?.id),
            None => None,
        };

        let character = character_repo
            .create(character_id, character, corporation_id, faction_id)
            .await?;

        Ok(character)
    }

    /// Get character from database or create an entry for it from ESI
    pub async fn get_or_create_character(
        &self,
        character_id: i64,
    ) -> Result<entity::eve_character::Model, Error> {
        let character_repo = CharacterRepository::new(&self.db);

        if let Some(character) = character_repo.get_by_character_id(character_id).await? {
            return Ok(character);
        }

        let character = self.create_character(character_id).await?;

        Ok(character)
    }

    /// Fetches a character from EVE Online's ESI and upserts to database
    pub async fn upsert_character(
        &self,
        character_id: i64,
    ) -> Result<entity::eve_character::Model, Error> {
        let character_repo = CharacterRepository::new(&self.db);
        let corporation_service = CorporationService::new(&self.db, &self.esi_client);
        let faction_service = FactionService::new(&self.db, &self.esi_client);

        // Get character information from ESI
        let character = self
            .esi_client
            .character()
            .get_character_public_information(character_id)
            .await?;

        // Ensure corporation exists in database or create it if applicable to prevent foreign key error
        let corporation_id = corporation_service
            .get_or_create_corporation(character.corporation_id)
            .await?
            .id;

        // Ensure faction exists in database or create it if applicable to prevent foreign key error
        let faction_id = match character.faction_id {
            Some(id) => Some(faction_service.get_or_update_factions(id).await?.id),
            None => None,
        };

        // Update or create character in database
        let character = character_repo
            .upsert(character_id, character, corporation_id, faction_id)
            .await?;

        Ok(character)
    }
}

#[cfg(test)]
mod tests {
    use bifrost_test_utils::prelude::*;

    use super::*;

    mod create_character {
        use super::*;

        /// Expect Ok when creating character without alliance or faction
        #[tokio::test]
        async fn creates_character_without_alliance_or_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test
                .eve()
                .with_character_endpoint(character_id, 1, None, None, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(result.is_ok());
            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when creating character with alliance
        #[tokio::test]
        async fn creates_character_with_alliance() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test
                .eve()
                .with_character_endpoint(character_id, 1, Some(1), None, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(result.is_ok());
            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when creating character with faction
        #[tokio::test]
        async fn creates_character_with_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test
                .eve()
                .with_character_endpoint(character_id, 1, None, Some(1), 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(result.is_ok());
            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when creating character with alliance & faction
        #[tokio::test]
        async fn creates_character_with_alliance_and_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints =
                test.eve()
                    .with_character_endpoint(character_id, 1, Some(1), Some(1), 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

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
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let character_id = 1;
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error when trying to create character that already exists
        #[tokio::test]
        async fn fails_for_duplicate_character() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let corporation_id = 1;
            let character_id = 1;
            let _ = test
                .eve()
                .insert_mock_character(character_id, corporation_id, None, None)
                .await?;
            let endpoints =
                test.eve()
                    .with_character_endpoint(character_id, corporation_id, None, None, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            // Assert only character endpoint was fetched prior to DB error
            endpoints.first().unwrap().assert();

            Ok(())
        }
    }

    mod get_or_create_character {
        use super::*;

        /// Expect Ok when character is found in database
        #[tokio::test]
        async fn finds_existing_character() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_or_create_character(character_model.character_id)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok when character is created when not found in database
        #[tokio::test]
        async fn creates_character_when_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test
                .eve()
                .with_character_endpoint(character_id, 1, None, None, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_or_create_character(character_id)
                .await;

            assert!(result.is_ok());
            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error when attempting to access database tables that haven't been created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let character_id = 1;
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_or_create_character(character_id)
                .await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        /// Expect Error when attempting to fetch from ESI endpoint that doesn't exist
        #[tokio::test]
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let character_id = 1;
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_or_create_character(character_id)
                .await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }
    }

    mod upsert_character {
        use chrono::{Duration, Utc};
        use sea_orm::{ActiveValue, EntityTrait, IntoActiveModel};

        use super::*;

        /// Expect Ok when upserting a new character with faction
        #[tokio::test]
        async fn creates_new_character_with_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test
                .eve()
                .with_character_endpoint(character_id, 1, None, Some(1), 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.upsert_character(character_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.character_id, character_id);
            assert!(created.faction_id.is_some());

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when upserting a new character without faction
        #[tokio::test]
        async fn creates_new_character_without_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test
                .eve()
                .with_character_endpoint(character_id, 1, None, None, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.upsert_character(character_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.character_id, character_id);
            assert_eq!(created.faction_id, None);

            // Assert 1 request was made to mock character endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when upserting an existing character with a new corporation ID
        #[tokio::test]
        async fn updates_character_corporation_relationship() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let corporation_model1 = test.eve().insert_mock_corporation(1, None, None).await?;
            let character_model = test
                .eve()
                .insert_mock_character(1, corporation_model1.corporation_id, None, None)
                .await?;

            // Mock endpoint returns character with different corporation
            let endpoints = test.eve().with_character_endpoint(1, 2, None, None, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .upsert_character(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
            assert_ne!(upserted.corporation_id, character_model.corporation_id);

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when upserting an existing character with a new faction ID
        #[tokio::test]
        async fn updates_character_faction_relationship() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_model = test
                .eve()
                .insert_mock_character(1, 1, None, Some(1))
                .await?;

            // Set faction last updated before today's faction update window to allow for updating
            // the faction from ESI
            let mut faction_model_am =
                entity::prelude::EveFaction::find_by_id(character_model.faction_id.unwrap())
                    .one(&test.state.db)
                    .await?
                    .unwrap()
                    .into_active_model();

            faction_model_am.updated_at =
                ActiveValue::Set((Utc::now() - Duration::hours(24)).naive_utc());

            entity::prelude::EveFaction::update(faction_model_am)
                .exec(&test.state.db)
                .await?;

            // Mock endpoint returns character with different faction
            let endpoints = test.eve().with_character_endpoint(1, 2, None, Some(2), 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .upsert_character(character_model.character_id)
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
            assert_ne!(upserted.faction_id, character_model.faction_id);

            // Assert 1 request was made to each mock endpoint
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
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let character_model = test
                .eve()
                .insert_mock_character(1, 1, None, Some(faction_model.faction_id))
                .await?;

            assert!(character_model.faction_id.is_some());

            // Mock endpoint returns character without faction
            let endpoints = test.eve().with_character_endpoint(1, 2, None, None, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .upsert_character(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
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
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

            assert_eq!(character_model.faction_id, None);

            // Mock endpoint returns character with faction
            let endpoints = test.eve().with_character_endpoint(1, 2, None, Some(1), 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .upsert_character(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
            assert!(upserted.faction_id.is_some());

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error when ESI endpoint for character is unavailable
        #[tokio::test]
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let character_id = 1;
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.upsert_character(character_id).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error due to required tables not being created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!()?;
            let endpoints = test.eve().with_character_endpoint(1, 1, None, None, 1);

            let character_id = 1;
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.upsert_character(character_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            // Assert only character endpoint was fetched prior to DB error
            endpoints.first().unwrap().assert();

            Ok(())
        }
    }
}
