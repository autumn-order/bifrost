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
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, DbBackend, DbErr, Schema};

    use crate::server::util::test::setup::{test_setup, TestSetup};

    async fn setup() -> Result<TestSetup, DbErr> {
        let test = test_setup().await;

        let db = &test.state.db;
        let schema = Schema::new(DbBackend::Sqlite);

        let stmts = vec![
            schema.create_table_from_entity(entity::prelude::EveFaction),
            schema.create_table_from_entity(entity::prelude::EveAlliance),
            schema.create_table_from_entity(entity::prelude::EveCorporation),
            schema.create_table_from_entity(entity::prelude::EveCharacter),
        ];

        for stmt in stmts {
            db.execute(&stmt).await?;
        }

        Ok(test)
    }

    mod create_character_tests {
        use crate::server::{
            error::Error,
            service::eve::character::{tests::setup, CharacterService},
            util::test::{
                eve::mock::{mock_character, mock_corporation, mock_faction},
                mockito::{
                    character::mock_character_endpoint, corporation::mock_corporation_endpoint,
                    faction::mock_faction_endpoint,
                },
                setup::test_setup,
            },
        };

        /// Expect success when creating a new character entry
        #[tokio::test]
        async fn test_create_character_success() -> Result<(), Error> {
            let mut test = setup().await?;
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);

            let alliance_id = None;
            let faction_id = None;
            let mock_corporation = mock_corporation(alliance_id, faction_id);

            let corporation_id = 1;
            let mock_character = mock_character(corporation_id, alliance_id, faction_id);

            let expected_requests = 1;
            let corporation_endpoint = mock_corporation_endpoint(
                &mut test.server,
                "/corporations/1",
                mock_corporation,
                expected_requests,
            );
            let character_endpoint = mock_character_endpoint(
                &mut test.server,
                "/characters/1",
                mock_character,
                expected_requests,
            );

            let character_id = 1;
            let result = character_service.create_character(character_id).await;

            assert!(result.is_ok());

            // Assert 1 request was made to each mock endpoint
            corporation_endpoint.assert();
            character_endpoint.assert();

            Ok(())
        }

        /// Expect success when creating a new character entry with an associated faction
        #[tokio::test]
        async fn test_create_character_with_faction_success() -> Result<(), Error> {
            let mut test = setup().await?;
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);

            // Character is member of faction, corporation is not
            let alliance_id = None;
            let faction_id = None;
            let mock_corporation = mock_corporation(alliance_id, faction_id);

            let faction_id = Some(0);
            let mock_faction = mock_faction();

            let corporation_id = 1;
            let mock_character = mock_character(corporation_id, alliance_id, faction_id);

            let expected_requests = 1;
            let corporation_endpoint = mock_corporation_endpoint(
                &mut test.server,
                "/corporations/1",
                mock_corporation,
                expected_requests,
            );
            let faction_endpoint =
                mock_faction_endpoint(&mut test.server, vec![mock_faction], expected_requests);
            let character_endpoint = mock_character_endpoint(
                &mut test.server,
                "/characters/1",
                mock_character,
                expected_requests,
            );

            let character_id = 1;
            let result = character_service.create_character(character_id).await;

            assert!(result.is_ok());

            // Assert 1 request was made to each mock endpoint
            corporation_endpoint.assert();
            faction_endpoint.assert();
            character_endpoint.assert();

            Ok(())
        }

        /// Expect Error when fetching character from an endpoint that doesn't exist
        #[tokio::test]
        async fn test_create_character_esi_error() -> Result<(), Error> {
            let test = setup().await?;
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);

            // Create no mock endpoints which will cause an ESI error

            let character_id = 1;
            let result = character_service.create_character(character_id).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error when trying to access database tables that don't exist
        #[tokio::test]
        async fn test_create_character_database_error() -> Result<(), Error> {
            // Use setup that doesn't create any required tables which will cause database error
            let mut test = test_setup().await;
            let character_service = CharacterService::new(&test.state.db, &test.state.esi_client);

            // Only create character endpoint, database error is returned before corporation endpoint is fetched
            let alliance_id = None;
            let faction_id = None;
            let corporation_id = 1;
            let mock_character = mock_character(corporation_id, alliance_id, faction_id);

            let expected_requests = 1;
            let character_endpoint = mock_character_endpoint(
                &mut test.server,
                "/characters/1",
                mock_character,
                expected_requests,
            );

            let character_id = 1;
            let result = character_service.create_character(character_id).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::DbErr(_))));

            // Assert 1 request was made to mock endpoint prior to database error
            character_endpoint.assert();

            Ok(())
        }
    }
}
