use eve_esi::model::character::Character;
use futures::future::join_all;
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

    /// Fetches a list of characters from ESI using their character IDs
    /// Makes concurrent requests in batches of up to 10 at a time
    // TODO: unit tests, need to fix some bifrost-test-utils mock endpoint issues first
    pub async fn get_many_characters(
        &self,
        character_ids: Vec<i64>,
    ) -> Result<Vec<(i64, Character)>, Error> {
        const BATCH_SIZE: usize = 10;
        let mut all_characters = Vec::new();

        // Process character IDs in chunks of BATCH_SIZE
        for chunk in character_ids.chunks(BATCH_SIZE) {
            // Create futures for all requests in this batch
            let futures: Vec<_> = chunk
                .iter()
                .map(|&character_id| async move {
                    let character = self
                        .esi_client
                        .character()
                        .get_character_public_information(character_id)
                        .await?;
                    Ok::<(i64, Character), Error>((character_id, character))
                })
                .collect();

            // Execute all futures in this batch concurrently
            let results = join_all(futures).await;

            // Collect results, propagating any errors
            for result in results {
                all_characters.push(result?);
            }
        }

        Ok(all_characters)
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

            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(1, corporation_id, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(result.is_ok());
            corporation_endpoint.assert();
            character_endpoint.assert();

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

            let alliance_id = 1;
            let corporation_id = 1;
            let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, None);
            let (_, mock_corporation) =
                test.eve()
                    .with_mock_corporation(corporation_id, Some(alliance_id), None);
            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(1, corporation_id, Some(alliance_id), None);

            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(result.is_ok());
            alliance_endpoint.assert();
            corporation_endpoint.assert();
            character_endpoint.assert();

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

            let faction_id = 1;
            let corporation_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (_, mock_corporation) =
                test.eve()
                    .with_mock_corporation(corporation_id, None, Some(faction_id));
            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(1, corporation_id, None, Some(faction_id));

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(result.is_ok());
            faction_endpoint.assert();
            corporation_endpoint.assert();
            character_endpoint.assert();

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

            let faction_id = 1;
            let alliance_id = 1;
            let corporation_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, Some(faction_id));
            let (_, mock_corporation) = test.eve().with_mock_corporation(
                corporation_id,
                Some(alliance_id),
                Some(faction_id),
            );
            let (character_id, mock_character) = test.eve().with_mock_character(
                1,
                corporation_id,
                Some(alliance_id),
                Some(faction_id),
            );

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(result.is_ok());
            faction_endpoint.assert();
            alliance_endpoint.assert();
            corporation_endpoint.assert();
            character_endpoint.assert();

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

            let (_, mock_character) =
                test.eve()
                    .with_mock_character(character_id, corporation_id, None, None);

            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.create_character(character_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));
            character_endpoint.assert();

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

            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(1, corporation_id, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_or_create_character(character_id)
                .await;

            assert!(result.is_ok());
            corporation_endpoint.assert();
            character_endpoint.assert();

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

    mod get_many_characters {
        use super::*;

        /// Expect Ok when fetching multiple characters successfully
        #[tokio::test]
        async fn fetches_multiple_characters() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Setup mock endpoints for 3 different characters
            let character_ids = vec![1, 2, 3];
            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let mut character_endpoints = Vec::new();
            for id in &character_ids {
                let (char_id, mock_character) =
                    test.eve()
                        .with_mock_character(*id, corporation_id, None, None);
                character_endpoints.push(test.eve().with_character_endpoint(
                    char_id,
                    mock_character,
                    1,
                ));
            }

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_many_characters(character_ids.clone())
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 3);

            // Verify all character IDs are present (order may vary due to concurrency)
            let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
            for id in &character_ids {
                assert!(returned_ids.contains(id));
            }

            corporation_endpoint.assert();
            for endpoint in character_endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok with empty vec when given empty character IDs list
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.get_many_characters(vec![]).await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 0);

            Ok(())
        }

        /// Expect Ok when fetching single character
        #[tokio::test]
        async fn fetches_single_character() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(1, corporation_id, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_many_characters(vec![character_id])
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 1);
            assert_eq!(characters[0].0, character_id);

            corporation_endpoint.assert();
            character_endpoint.assert();

            Ok(())
        }

        /// Expect Ok when fetching characters with various relationships
        #[tokio::test]
        async fn fetches_characters_with_relationships() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let faction_id = 1;
            let alliance_id = 1;
            let corporation_id = 1;

            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (_, mock_alliance) = test.eve().with_mock_alliance(alliance_id, Some(faction_id));
            let (_, mock_corporation) = test.eve().with_mock_corporation(
                corporation_id,
                Some(alliance_id),
                Some(faction_id),
            );
            let (character_id, mock_character) = test.eve().with_mock_character(
                1,
                corporation_id,
                Some(alliance_id),
                Some(faction_id),
            );

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_many_characters(vec![character_id])
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 1);
            assert_eq!(characters[0].1.faction_id, Some(faction_id));
            assert_eq!(characters[0].1.alliance_id, Some(alliance_id));

            faction_endpoint.assert();
            alliance_endpoint.assert();
            corporation_endpoint.assert();
            character_endpoint.assert();

            Ok(())
        }

        /// Expect Error when ESI endpoint is unavailable for any character
        #[tokio::test]
        async fn fails_when_esi_unavailable() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let character_ids = vec![1, 2, 3];
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.get_many_characters(character_ids).await;

            // Should fail on first unavailable character
            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error when ESI fails partway through batch
        #[tokio::test]
        async fn fails_on_partial_esi_failure() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(1, corporation_id, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_ids = vec![1, 2, 3];
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.get_many_characters(character_ids).await;

            // Should succeed on first, fail on second (no mock)
            assert!(matches!(result, Err(Error::EsiError(_))));

            corporation_endpoint.assert();
            character_endpoint.assert();

            Ok(())
        }

        /// Expect Ok when fetching many characters (stress test)
        #[tokio::test]
        async fn fetches_many_characters() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Setup mock endpoints for 10 characters
            let character_ids: Vec<i64> = (1..=10).collect();
            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let mut character_endpoints = Vec::new();
            for id in &character_ids {
                let (char_id, mock_character) =
                    test.eve()
                        .with_mock_character(*id, corporation_id, None, None);
                character_endpoints.push(test.eve().with_character_endpoint(
                    char_id,
                    mock_character,
                    1,
                ));
            }

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_many_characters(character_ids.clone())
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 10);

            // Verify all character IDs are present (order may vary due to concurrency)
            let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
            for id in &character_ids {
                assert!(returned_ids.contains(id));
            }

            corporation_endpoint.assert();
            for endpoint in character_endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Ok when fetching more than 10 characters (tests batching)
        #[tokio::test]
        async fn fetches_many_characters_with_batching() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Setup mock endpoints for 25 characters to test multiple batches
            let character_ids: Vec<i64> = (1..=25).collect();
            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let mut character_endpoints = Vec::new();
            for id in &character_ids {
                let (char_id, mock_character) =
                    test.eve()
                        .with_mock_character(*id, corporation_id, None, None);
                character_endpoints.push(test.eve().with_character_endpoint(
                    char_id,
                    mock_character,
                    1,
                ));
            }

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_many_characters(character_ids.clone())
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 25);

            // Verify all character IDs are present (order may vary due to concurrency)
            let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
            for id in &character_ids {
                assert!(returned_ids.contains(id));
            }

            corporation_endpoint.assert();
            for endpoint in character_endpoints {
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
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Setup mock endpoints for 5 characters (within one batch)
            let character_ids: Vec<i64> = (1..=5).collect();
            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let mut character_endpoints = Vec::new();
            for id in &character_ids {
                let (char_id, mock_character) =
                    test.eve()
                        .with_mock_character(*id, corporation_id, None, None);
                character_endpoints.push(test.eve().with_character_endpoint(
                    char_id,
                    mock_character,
                    1,
                ));
            }

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_many_characters(character_ids.clone())
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 5);

            // Verify all character IDs are present
            let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
            for id in &character_ids {
                assert!(returned_ids.contains(id));
            }

            corporation_endpoint.assert();
            for endpoint in character_endpoints {
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
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(1, corporation_id, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_ids = vec![1, 2, 3, 4, 5];
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.get_many_characters(character_ids).await;

            // Should fail when any request in the batch fails
            assert!(matches!(result, Err(Error::EsiError(_))));

            corporation_endpoint.assert();
            character_endpoint.assert();

            Ok(())
        }

        /// Expect correct batching behavior with exactly 10 items
        #[tokio::test]
        async fn handles_exact_batch_size() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Setup mock endpoints for exactly 10 characters (one full batch)
            let character_ids: Vec<i64> = (1..=10).collect();
            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let mut character_endpoints = Vec::new();
            for id in &character_ids {
                let (char_id, mock_character) =
                    test.eve()
                        .with_mock_character(*id, corporation_id, None, None);
                character_endpoints.push(test.eve().with_character_endpoint(
                    char_id,
                    mock_character,
                    1,
                ));
            }

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_many_characters(character_ids.clone())
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 10);

            // Verify all character IDs are present
            let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
            for id in &character_ids {
                assert!(returned_ids.contains(id));
            }

            corporation_endpoint.assert();
            for endpoint in character_endpoints {
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
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Setup mock endpoints for 11 characters (one full batch + one item)
            let character_ids: Vec<i64> = (1..=11).collect();
            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let mut character_endpoints = Vec::new();
            for id in &character_ids {
                let (char_id, mock_character) =
                    test.eve()
                        .with_mock_character(*id, corporation_id, None, None);
                character_endpoints.push(test.eve().with_character_endpoint(
                    char_id,
                    mock_character,
                    1,
                ));
            }

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .get_many_characters(character_ids.clone())
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 11);

            // Verify all character IDs are present
            let returned_ids: Vec<i64> = characters.iter().map(|(id, _)| *id).collect();
            for id in &character_ids {
                assert!(returned_ids.contains(id));
            }

            corporation_endpoint.assert();
            for endpoint in character_endpoints {
                endpoint.assert();
            }

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

            let faction_id = 1;
            let corporation_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (_, mock_corporation) =
                test.eve()
                    .with_mock_corporation(corporation_id, None, Some(faction_id));
            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(1, corporation_id, None, Some(faction_id));

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.upsert_character(character_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.character_id, character_id);
            assert!(created.faction_id.is_some());

            faction_endpoint.assert();
            corporation_endpoint.assert();
            character_endpoint.assert();

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

            let corporation_id = 1;
            let (_, mock_corporation) =
                test.eve().with_mock_corporation(corporation_id, None, None);
            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(1, corporation_id, None, None);

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.upsert_character(character_id).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.character_id, character_id);
            assert_eq!(created.faction_id, None);

            corporation_endpoint.assert();
            character_endpoint.assert();

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
            let corporation_id_2 = 2;
            let (_, mock_corporation_2) =
                test.eve()
                    .with_mock_corporation(corporation_id_2, None, None);
            let (_, mock_character) = test.eve().with_mock_character(
                character_model.character_id,
                corporation_id_2,
                None,
                None,
            );

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_model.character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .upsert_character(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
            assert_ne!(upserted.corporation_id, character_model.corporation_id);

            corporation_endpoint.assert();
            character_endpoint.assert();

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
            let faction_id_2 = 2;
            let corporation_id_2 = 2;
            let mock_faction_2 = test.eve().with_mock_faction(faction_id_2);
            let (_, mock_corporation_2) =
                test.eve()
                    .with_mock_corporation(corporation_id_2, None, Some(faction_id_2));
            let (_, mock_character) = test.eve().with_mock_character(
                character_model.character_id,
                corporation_id_2,
                None,
                Some(faction_id_2),
            );

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction_2], 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_model.character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .upsert_character(character_model.character_id)
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
            assert_ne!(upserted.faction_id, character_model.faction_id);

            faction_endpoint.assert();
            corporation_endpoint.assert();
            character_endpoint.assert();

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
            let corporation_id_2 = 2;
            let (_, mock_corporation_2) =
                test.eve()
                    .with_mock_corporation(corporation_id_2, None, None);
            let (_, mock_character) = test.eve().with_mock_character(
                character_model.character_id,
                corporation_id_2,
                None,
                None,
            );

            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_model.character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .upsert_character(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
            assert_eq!(upserted.faction_id, None);

            corporation_endpoint.assert();
            character_endpoint.assert();

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
            let faction_id = 1;
            let corporation_id_2 = 2;
            let mock_faction = test.eve().with_mock_faction(faction_id);
            let (_, mock_corporation_2) =
                test.eve()
                    .with_mock_corporation(corporation_id_2, None, Some(faction_id));
            let (_, mock_character) = test.eve().with_mock_character(
                character_model.character_id,
                corporation_id_2,
                None,
                Some(faction_id),
            );

            let faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id_2, mock_corporation_2, 1);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_model.character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service
                .upsert_character(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
            assert!(upserted.faction_id.is_some());

            faction_endpoint.assert();
            corporation_endpoint.assert();
            character_endpoint.assert();

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

            let corporation_id = 1;
            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(1, corporation_id, None, None);

            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);
            let result = character_service.upsert_character(character_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));
            character_endpoint.assert();

            Ok(())
        }
    }
}
