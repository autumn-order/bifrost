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
}

#[cfg(test)]
mod tests {

    mod create_character {
        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::eve::character::CharacterService};

        /// Expect Ok when creating character without alliance or faction
        #[tokio::test]
        async fn create_character_ok_no_alliance_or_faction() -> Result<(), TestError> {
            let mut test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test.with_character_endpoint(character_id, 1, None, None, 1);

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
        async fn create_character_ok_with_alliance() -> Result<(), TestError> {
            let mut test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test.with_character_endpoint(character_id, 1, Some(1), None, 1);

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
        async fn create_character_ok_with_faction() -> Result<(), TestError> {
            let mut test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test.with_character_endpoint(character_id, 1, None, Some(1), 1);

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
        async fn create_character_ok_with_alliance_and_faction() -> Result<(), TestError> {
            let mut test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test.with_character_endpoint(character_id, 1, Some(1), Some(1), 1);

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
        async fn create_character_err_esi() -> Result<(), TestError> {
            let test = test_setup!(
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
        async fn create_character_err_duplicate_character() -> Result<(), TestError> {
            let mut test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let corporation_id = 1;
            let character_id = 1;
            let _ = test
                .insert_mock_character(character_id, corporation_id, None, None)
                .await?;
            let endpoints =
                test.with_character_endpoint(character_id, corporation_id, None, None, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::DbErr(_))));
            // Assert 1 request was made to mock endpoint
            //
            // Use last() to assert only the character endpoint since DB error occurs
            // afterwards when trying to get_or_create_corporation
            endpoints.last().unwrap().assert();

            Ok(())
        }
    }

    mod get_or_create_character {
        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::eve::character::CharacterService};

        /// Expect Ok when character is found in database
        #[tokio::test]
        async fn get_or_create_character_ok_found() -> Result<(), TestError> {
            let test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_model = test.insert_mock_character(1, 1, None, None).await?;

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_or_create_character(character_model.character_id)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok when character is created when not found in database
        #[tokio::test]
        async fn test_get_or_create_character_created() -> Result<(), TestError> {
            let mut test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_id = 1;
            let endpoints = test.with_character_endpoint(character_id, 1, None, None, 1);

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
        async fn get_or_create_character_err_missing_tables() -> Result<(), TestError> {
            let test = test_setup!()?;

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
        async fn get_or_create_character_err_esi() -> Result<(), TestError> {
            let test = test_setup!(
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
}
