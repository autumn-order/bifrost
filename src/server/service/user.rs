use sea_orm::DatabaseConnection;

use crate::server::{
    data::{
        eve::character::CharacterRepository,
        user::{user::UserRepository, user_character::UserCharacterRepository},
    },
    error::Error,
    service::eve::character::CharacterService,
};

pub struct UserService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> UserService<'a> {
    /// Creates a new instance of [`UserService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    pub async fn get_or_create_user(&self, character_id: i64) -> Result<i32, Error> {
        let user_repo = UserRepository::new(&self.db);
        let character_repo = CharacterRepository::new(&self.db);
        let user_character_repo = UserCharacterRepository::new(&self.db);
        let character_service = CharacterService::new(&self.db, &self.esi_client);

        let character = match character_repo.get_by_character_id(character_id).await? {
            Some(character) => {
                if let Some(character_owner) = user_character_repo
                    .get_by_character_id(character.id)
                    .await?
                {
                    return Ok(character_owner.id);
                }

                character
            }
            None => character_service.create_character(character_id).await?,
        };

        let new_user = user_repo.create().await?;
        let _ = user_character_repo
            .create(new_user.id, character.id)
            .await?;

        Ok(new_user.id)
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
            schema.create_table_from_entity(entity::prelude::BifrostUser),
            schema.create_table_from_entity(entity::prelude::BifrostUserCharacter),
        ];

        for stmt in stmts {
            db.execute(&stmt).await?;
        }

        Ok(test)
    }

    mod get_or_create_user_tests {
        use crate::server::{
            data::{
                eve::{character::CharacterRepository, corporation::CorporationRepository},
                user::{user::UserRepository, user_character::UserCharacterRepository},
            },
            error::Error,
            service::user::{tests::setup, UserService},
            util::test::{
                eve::mock::{mock_character, mock_corporation},
                mockito::{
                    character::mock_character_endpoint, corporation::mock_corporation_endpoint,
                },
                setup::test_setup,
            },
        };

        /// Expect success when user associated with character is found
        #[tokio::test]
        async fn test_get_or_create_user_found_user() -> Result<(), Error> {
            let test = setup().await?;

            let character_repo = CharacterRepository::new(&test.state.db);
            let corporation_repo = CorporationRepository::new(&test.state.db);
            let user_repo = UserRepository::new(&test.state.db);
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let faction_id = None;
            let alliance_id = None;
            let corporation_id = 1;
            let mock_corporation = mock_corporation(alliance_id, faction_id);

            let character_id = 1;
            let mock_character = mock_character(corporation_id, alliance_id, faction_id);

            let corporation = corporation_repo
                .create(corporation_id, mock_corporation, None, None)
                .await?;
            let character = character_repo
                .create(character_id, mock_character, corporation.id, None)
                .await?;
            let user = user_repo.create().await?;
            let _ = user_character_repo.create(user.id, character.id).await?;

            let result = user_service.get_or_create_user(character_id).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect success when character is found but new user is created
        #[tokio::test]
        async fn test_get_or_create_user_new_user() -> Result<(), Error> {
            let test = setup().await?;

            let character_repo = CharacterRepository::new(&test.state.db);
            let corporation_repo = CorporationRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let faction_id = None;
            let alliance_id = None;
            let corporation_id = 1;
            let mock_corporation = mock_corporation(alliance_id, faction_id);

            let character_id = 1;
            let mock_character = mock_character(corporation_id, alliance_id, faction_id);

            let corporation = corporation_repo
                .create(corporation_id, mock_corporation, None, None)
                .await?;
            let _ = character_repo
                .create(character_id, mock_character, corporation.id, None)
                .await?;

            let result = user_service.get_or_create_user(character_id).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect success when new character & user is created
        #[tokio::test]
        async fn test_get_or_create_user_new_character() -> Result<(), Error> {
            let mut test = setup().await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let faction_id = None;
            let alliance_id = None;
            let corporation_id = 1;
            let mock_corporation = mock_corporation(alliance_id, faction_id);

            let character_id = 1;
            let mock_character = mock_character(corporation_id, alliance_id, faction_id);

            let mock_corporation_endpoint =
                mock_corporation_endpoint(&mut test.server, "/corporations/1", mock_corporation, 1);
            let mock_character_endpoint =
                mock_character_endpoint(&mut test.server, "/characters/1", mock_character, 1);

            let result = user_service.get_or_create_user(character_id).await;

            assert!(result.is_ok());

            // Assert 1 request was made to each mock endpoint
            mock_corporation_endpoint.assert();
            mock_character_endpoint.assert();

            Ok(())
        }

        /// Expect Error when the required database tables haven't been created
        #[tokio::test]
        async fn test_get_or_create_user_database_error() -> Result<(), Error> {
            // Use test setup that doesn't create required tables, causing database error
            let test = test_setup().await;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let character_id = 1;
            let result = user_service.get_or_create_user(character_id).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        /// Expect Error when required ESI endpoints are unavailable
        #[tokio::test]
        async fn test_get_or_create_user_esi_error() -> Result<(), Error> {
            let test = setup().await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            // Don't create mock ESI endpoints, causing an ESI error

            let character_id = 1;
            let result = user_service.get_or_create_user(character_id).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }
    }
}
