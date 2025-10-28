use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, DatabaseConnection, DbErr, DeleteResult, EntityTrait,
    IntoActiveModel,
};

pub struct UserRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserRepository<'a> {
    /// Creates a new instance of [`UserRepository`]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// Creates a new user
    pub async fn create(
        &self,
        main_character_id: i32,
    ) -> Result<entity::bifrost_user::Model, DbErr> {
        let user = entity::bifrost_user::ActiveModel {
            main_character_id: ActiveValue::Set(main_character_id),
            created_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        user.insert(self.db).await
    }

    pub async fn get(
        &self,
        user_id: i32,
    ) -> Result<
        Option<(
            entity::bifrost_user::Model,
            Option<entity::eve_character::Model>,
        )>,
        DbErr,
    > {
        entity::prelude::BifrostUser::find_by_id(user_id)
            .find_also_related(entity::eve_character::Entity)
            .one(self.db)
            .await
    }

    pub async fn update(
        &self,
        user_id: i32,
        new_main_character_id: i32,
    ) -> Result<Option<entity::bifrost_user::Model>, DbErr> {
        let user = match entity::prelude::BifrostUser::find_by_id(user_id)
            .one(self.db)
            .await?
        {
            Some(user) => user,
            None => return Ok(None),
        };

        let mut user_am = user.into_active_model();
        user_am.main_character_id = ActiveValue::Set(new_main_character_id);

        let user = user_am.update(self.db).await?;

        Ok(Some(user))
    }

    /// Deletes a user
    ///
    /// Returns OK regardless of user existing, to confirm the deletion result
    /// check the [`DeleteResult::rows_affected`] field.
    pub async fn delete(&self, user_id: i32) -> Result<DeleteResult, DbErr> {
        entity::prelude::BifrostUser::delete_by_id(user_id)
            .exec(self.db)
            .await
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Schema};

    use crate::server::{
        error::Error,
        util::test::setup::{
            test_setup, test_setup_create_character, test_setup_create_corporation,
        },
    };

    async fn setup() -> Result<(DatabaseConnection, entity::eve_character::Model), Error> {
        let test = test_setup().await;

        let db = &test.state.db;
        let schema = Schema::new(DbBackend::Sqlite);

        let stmts = vec![
            schema.create_table_from_entity(entity::prelude::EveFaction),
            schema.create_table_from_entity(entity::prelude::EveAlliance),
            schema.create_table_from_entity(entity::prelude::EveCorporation),
            schema.create_table_from_entity(entity::prelude::EveCharacter),
            schema.create_table_from_entity(entity::prelude::BifrostUser),
        ];

        for stmt in stmts {
            db.execute(&stmt).await?;
        }

        let corporation_id = 1;
        let character_id = 1;
        let corporation = test_setup_create_corporation(&test, corporation_id).await?;
        let character = test_setup_create_character(&test, character_id, corporation).await?;

        Ok((test.state.db, character))
    }

    mod create_tests {
        use crate::server::{
            data::user::user::{tests::setup, UserRepository},
            error::Error,
        };

        /// Expect success when creating a new user
        #[tokio::test]
        async fn test_create_user_success() -> Result<(), Error> {
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);

            let result = user_repository.create(character.id).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Error when setting user main character to character that does not exist in database
        #[tokio::test]
        async fn test_create_user_error() -> Result<(), Error> {
            let (db, _) = setup().await?;
            let user_repository = UserRepository::new(&db);

            let non_existant_main_character_id = 2;
            let result = user_repository.create(non_existant_main_character_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod get_tests {
        use crate::server::{
            data::user::user::{tests::setup, UserRepository},
            error::Error,
            util::test::setup::test_setup,
        };

        /// Expect Ok(Some(_)) when existing user is found
        #[tokio::test]
        async fn get_user_some_with_existing_user() -> Result<(), Error> {
            let (db, character) = setup().await?;
            let user_repo = UserRepository::new(&db);
            let user = user_repo.create(character.id).await?;

            let result = user_repo.get(user.id).await;
            assert!(matches!(result, Ok(Some(_))));

            Ok(())
        }

        /// Expect Ok(None) when user is not found
        #[tokio::test]
        async fn get_user_some_with_non_existant_user() -> Result<(), Error> {
            let (db, _) = setup().await?;
            let user_repo = UserRepository::new(&db);

            let non_existant_user_id = 1;
            let result = user_repo.get(non_existant_user_id).await;
            assert!(matches!(result, Ok(None)));

            Ok(())
        }

        /// Expect Error when required database tables are not present]
        #[tokio::test]
        async fn get_user_error_with_missing_tables() -> Result<(), Error> {
            let test = test_setup().await;
            let user_repo = UserRepository::new(&test.state.db);

            let user_id = 1;
            let result = user_repo.get(user_id).await;
            assert!(result.is_err());

            Ok(())
        }
    }

    mod update_tests {
        use crate::server::{
            data::{
                eve::character::CharacterRepository,
                user::user::{tests::setup, UserRepository},
            },
            error::Error,
            util::test::eve::mock::mock_character,
        };

        /// Expect Ok when updating user main character with valid character ID
        #[tokio::test]
        async fn update_user_some_with_existing_user() -> Result<(), Error> {
            let (db, character) = setup().await?;
            let user_repo = UserRepository::new(&db);
            let character_repo = CharacterRepository::new(&db);
            let user = user_repo.create(character.id).await?;

            let corporation_id = 1;
            let second_character = mock_character(corporation_id, None, None);
            let second_character_model =
                character_repo.create(2, second_character, 1, None).await?;

            let result = user_repo.update(user.id, second_character_model.id).await;
            assert!(matches!(result, Ok(Some(_))));
            let updated_user = result.unwrap().unwrap();
            assert_ne!(user.main_character_id, updated_user.main_character_id);

            Ok(())
        }

        /// Expect Ok(None) when attempting to update user ID that does not exist
        #[tokio::test]
        async fn update_user_none_with_non_existant_user() -> Result<(), Error> {
            let (db, character) = setup().await?;
            let user_repo = UserRepository::new(&db);

            let non_existant_user_id = 1;
            let result = user_repo.update(non_existant_user_id, character.id).await;
            assert!(matches!(result, Ok(None)));

            Ok(())
        }

        /// Expect Error when attempting to update user main character with non existant character ID
        #[tokio::test]
        async fn update_user_error_with_non_existant_character_id() -> Result<(), Error> {
            let (db, character) = setup().await?;
            let user_repo = UserRepository::new(&db);
            let user = user_repo.create(character.id).await?;

            let result = user_repo.update(user.id, character.id + 1).await;
            assert!(result.is_err());

            Ok(())
        }
    }

    mod delete_tests {
        use sea_orm::EntityTrait;

        use crate::server::{
            data::user::user::{tests::setup, UserRepository},
            error::Error,
            util::test::setup::test_setup,
        };

        /// Expect success when deleting user
        #[tokio::test]
        async fn test_delete_user_success() -> Result<(), Error> {
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);

            let user = user_repository.create(character.id).await?;

            let result = user_repository.delete(user.id).await;

            assert!(result.is_ok());
            let delete_result = result.unwrap();

            assert_eq!(delete_result.rows_affected, 1);

            // Ensure user has actually been deleted
            let user_exists = entity::prelude::BifrostUser::find_by_id(user.id)
                .one(&db)
                .await?;

            assert!(user_exists.is_none());

            Ok(())
        }

        /// Expect no rows to be affected when deleting user that does not exist
        #[tokio::test]
        async fn test_delete_user_none() -> Result<(), Error> {
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);

            let user = user_repository.create(character.id).await?;

            let result = user_repository.delete(user.id + 1).await;

            assert!(result.is_ok());
            let delete_result = result.unwrap();

            assert_eq!(delete_result.rows_affected, 0);

            Ok(())
        }

        /// Expect Error when database tables required don't exist
        #[tokio::test]
        async fn test_delete_user_error() -> Result<(), Error> {
            // Use test setup that doesn't create required tables, causing an error
            let test = test_setup().await;
            let user_repository = UserRepository::new(&test.state.db);

            let result = user_repository.delete(1).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
