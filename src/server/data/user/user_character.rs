use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, DbErr};

pub struct UserCharacterRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserCharacterRepository<'a> {
    /// Creates a new instance of [`UserCharacterRepository`]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// Create a new entry for a character owned by a user
    ///
    /// # Arguments
    /// - `user_id` (`i32`): ID of the user entry in the database
    /// - `character_id` (`i32`): ID of the character entry in the database
    pub async fn create(
        &self,
        user_id: i32,
        character_id: i32,
    ) -> Result<entity::bifrost_user_character::Model, DbErr> {
        let user_character = entity::bifrost_user_character::ActiveModel {
            user_id: ActiveValue::Set(user_id),
            character_id: ActiveValue::Set(character_id),
            created_at: ActiveValue::Set(Utc::now().naive_utc()),
            updated_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        user_character.insert(self.db).await
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Schema};

    use crate::server::{
        data::eve::{character::CharacterRepository, corporation::CorporationRepository},
        util::test::{
            eve::mock::{mock_character, mock_corporation},
            setup::test_setup,
        },
    };

    async fn setup() -> Result<(DatabaseConnection, i32), DbErr> {
        let test = test_setup().await;
        let db = test.state.db;

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

        Ok((db, character.id))
    }

    mod create_tests {
        use sea_orm::{DbErr, RuntimeErr};

        use crate::server::data::user::{
            user::UserRepository,
            user_character::{tests::setup, UserCharacterRepository},
        };

        /// Expect success when creating user character linked to existing user and character
        #[tokio::test]
        async fn test_create_user_character_success() -> Result<(), DbErr> {
            let (db, character_id) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            let user = user_repository.create().await?;
            let result = user_character_repository
                .create(user.id, character_id)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect error when creating user character linked to missing user
        #[tokio::test]
        async fn test_create_user_character_missing_user() -> Result<(), DbErr> {
            let (db, character_id) = setup().await?;
            let user_character_repository = UserCharacterRepository::new(&db);

            // Don't create a user first, this will cause a foreign key error
            let user_id = 1;
            let result = user_character_repository
                .create(user_id, character_id)
                .await;

            assert!(result.is_err());

            assert!(matches!(
                result,
                Err(DbErr::Query(RuntimeErr::SqlxError(err))) if err
                    .as_database_error()
                    .and_then(|d| d.code().map(|c| c == "787"))
                    .unwrap_or(false)
            ));

            Ok(())
        }

        /// Expect error when creating user character linked to missing character
        #[tokio::test]
        async fn test_create_user_character_missing_character() -> Result<(), DbErr> {
            let (db, character_id) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            // Increment character ID to one that does not exist, causing a foreign key error
            let user = user_repository.create().await?;
            let result = user_character_repository
                .create(user.id, character_id + 1)
                .await;

            assert!(result.is_err());

            // Assert error code is 787 indicating a foreign key constraint error
            assert!(matches!(
                result,
                Err(DbErr::Query(RuntimeErr::SqlxError(err))) if err
                    .as_database_error()
                    .and_then(|d| d.code().map(|c| c == "787"))
                    .unwrap_or(false)
            ));

            Ok(())
        }
    }
}
