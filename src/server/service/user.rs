use sea_orm::DatabaseConnection;

use crate::server::{
    data::user::{user::UserRepository, user_character::UserCharacterRepository},
    error::Error,
};

pub struct UserService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserService<'a> {
    /// Creates a new instance of [`UserService`]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    // Get or create a user based upon the provided Bifrost character ID
    //
    // Will check to see if the provided Bifrost character ID is owned by any
    // user, if not then a new user will be created and the character linked to
    // that user.
    //
    // # Arguments
    // - `character_id` (`i32`): The Bifrost character ID to find the user for
    //
    // # Returns
    // Returns a Result containing:
    // - `i32`: The ID of the user that was found or created
    // - [`Error`]: An error if there is an issue with the database
    pub async fn get_or_create_user(&self, character_id: i32) -> Result<i32, Error> {
        let user_character_repository = UserCharacterRepository::new(&self.db);
        let user_repository = UserRepository::new(&self.db);

        let user = user_character_repository
            .get_by_character_id(character_id)
            .await?;

        if let Some(user) = user {
            return Ok(user.id);
        }

        let new_user = user_repository.create().await?;

        // Link the character to the new user
        let _ = user_character_repository
            .create(new_user.id, character_id)
            .await?;

        Ok(new_user.id)
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, DbBackend, DbErr, Schema};

    use crate::server::{
        data::eve::{character::CharacterRepository, corporation::CorporationRepository},
        util::test::{
            eve::mock::{mock_character, mock_corporation},
            setup::{test_setup, TestSetup},
        },
    };

    async fn setup() -> Result<(TestSetup, i32), DbErr> {
        let test = test_setup().await;
        let db = &test.state.db;

        let character_repository = CharacterRepository::new(&db);
        let corporation_repository = CorporationRepository::new(&db);

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

        // Insert mock character & corporation required for tests
        let faction_id = None;
        let alliance_id = None;
        let corporation_id = 1;
        let mock_corporation = mock_corporation(alliance_id, faction_id);

        let character_id = 1;
        let mock_character = mock_character(corporation_id, alliance_id, faction_id);

        let corporation = corporation_repository
            .create(corporation_id, mock_corporation, None, None)
            .await?;
        let character = character_repository
            .create(character_id, mock_character, corporation.id, None)
            .await?;

        Ok((test, character.id))
    }

    mod get_or_create_user_tests {
        use sea_orm::DbErr;

        use crate::server::{
            data::user::{user::UserRepository, user_character::UserCharacterRepository},
            error::Error,
            service::user::{tests::setup, UserService},
            util::test::setup::test_setup,
        };

        // Expect success when user is already present in database
        #[tokio::test]
        async fn test_get_or_create_user_found() -> Result<(), DbErr> {
            let (test, character_id) = setup().await?;
            let user_repo = UserRepository::new(&test.state.db);
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db);

            let existing_user = user_repo.create().await?;
            let _ = user_character_repo
                .create(existing_user.id, character_id)
                .await?;

            let result = user_service.get_or_create_user(character_id).await;

            assert!(result.is_ok());

            Ok(())
        }

        // Expect success when a new user is created
        #[tokio::test]
        async fn test_get_or_create_user_created() -> Result<(), DbErr> {
            let (test, character_id) = setup().await?;
            let user_service = UserService::new(&test.state.db);

            let result = user_service.get_or_create_user(character_id).await;

            assert!(result.is_ok());

            Ok(())
        }

        // Expect error when required database tables haven't been created
        #[tokio::test]
        async fn test_get_or_create_user_error() -> Result<(), DbErr> {
            let test = test_setup().await;
            let user_service = UserService::new(&test.state.db);

            let character_id = 1;
            let result = user_service.get_or_create_user(character_id).await;

            assert!(result.is_err());

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }
    }
}
