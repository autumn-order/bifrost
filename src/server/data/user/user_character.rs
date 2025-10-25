use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    IntoActiveModel, QueryFilter,
};

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

    /// Get a user character entry using their Bifrost character ID
    pub async fn get_by_character_id(
        &self,
        character_id: i32,
    ) -> Result<Option<entity::bifrost_user_character::Model>, DbErr> {
        entity::prelude::BifrostUserCharacter::find()
            .filter(entity::bifrost_user_character::Column::CharacterId.eq(character_id))
            .one(self.db)
            .await
    }

    /// Update a user character entry with a new user id
    ///
    /// # Arguments
    /// - `user_character_entry_id`: The ID of the user character entry to update
    /// - `new_user_id`: The ID of the user to change the entry to
    ///
    /// # Returns
    /// Returns a result containing:
    /// - `Option<`[`entity::bifrost_user_character::Model`]`>`: Some if update is successful
    ///   or None if entry not found
    /// - [`DbErr`]: If a database-related error occurs
    pub async fn update(
        &self,
        user_character_entry_id: i32,
        new_user_id: i32,
    ) -> Result<Option<entity::bifrost_user_character::Model>, DbErr> {
        let user_character =
            match entity::prelude::BifrostUserCharacter::find_by_id(user_character_entry_id)
                .one(self.db)
                .await?
            {
                Some(user_character) => user_character,
                None => return Ok(None),
            };

        let mut user_character_am = user_character.into_active_model();
        user_character_am.user_id = ActiveValue::Set(new_user_id);
        user_character_am.updated_at = ActiveValue::Set(Utc::now().naive_utc());

        let user_character = user_character_am.update(self.db).await?;

        Ok(Some(user_character))
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

    mod get_by_character_id_tests {
        use sea_orm::DbErr;

        use crate::server::{
            data::user::{
                user::UserRepository,
                user_character::{tests::setup, UserCharacterRepository},
            },
            util::test::setup::test_setup,
        };

        // Expect Some when user character entry is present
        #[tokio::test]
        async fn test_get_by_character_id_some() -> Result<(), DbErr> {
            let (db, character_id) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            let user = user_repository.create().await?;
            let _ = user_character_repository
                .create(user.id, character_id)
                .await?;

            let result = user_character_repository
                .get_by_character_id(character_id)
                .await;

            assert!(result.is_ok());
            let character_option = result.unwrap();

            assert!(character_option.is_some());

            Ok(())
        }

        // Expect None when user character entry is not found
        #[tokio::test]
        async fn test_get_by_character_id_none() -> Result<(), DbErr> {
            let (db, character_id) = setup().await?;
            let user_character_repository = UserCharacterRepository::new(&db);

            let result = user_character_repository
                .get_by_character_id(character_id)
                .await;

            assert!(result.is_ok());
            let character_option = result.unwrap();

            assert!(character_option.is_none());

            Ok(())
        }

        // Expect Error when required database tables are not present
        #[tokio::test]
        async fn test_get_by_character_id_error() -> Result<(), DbErr> {
            // Use test setup that does not create required tables, causing a database error
            let test = test_setup().await;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);

            let character_id = 1;
            let result = user_character_repository
                .get_by_character_id(character_id)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod update_tests {
        use sea_orm::{DbErr, RuntimeErr};

        use crate::server::data::user::{
            user::UserRepository,
            user_character::{tests::setup, UserCharacterRepository},
        };

        /// Expect Some when user character update is successful
        #[tokio::test]
        async fn test_update_user_character_some() -> Result<(), DbErr> {
            let (db, character_id) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            let old_user = user_repository.create().await?;
            let user_character_entry = user_character_repository
                .create(old_user.id, character_id)
                .await?;

            let new_user = user_repository.create().await?;
            let result = user_character_repository
                .update(user_character_entry.id, new_user.id)
                .await;

            assert!(result.is_ok());
            let result_option = result.unwrap();

            assert!(result_option.is_some());

            Ok(())
        }

        /// Expect None when user character entry is not found
        #[tokio::test]
        async fn test_update_user_character_none() -> Result<(), DbErr> {
            let (db, _) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            // Try to update entry ID that doesn't exist
            let user_character_entry_id = 1;
            let new_user = user_repository.create().await?;
            let result = user_character_repository
                .update(user_character_entry_id, new_user.id)
                .await;

            assert!(result.is_ok());
            let result_option = result.unwrap();

            assert!(result_option.is_none());

            Ok(())
        }

        /// Expect Error when updating user character entry to user that doesn't exist
        #[tokio::test]
        async fn test_update_user_character_error() -> Result<(), DbErr> {
            let (db, character_id) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            let old_user = user_repository.create().await?;
            let user_character_entry = user_character_repository
                .create(old_user.id, character_id)
                .await?;

            // Try to update entry to new_user_id that doesn't exist
            let new_user_id = old_user.id + 1;
            let result = user_character_repository
                .update(user_character_entry.id, new_user_id)
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
