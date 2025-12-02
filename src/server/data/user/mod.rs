pub mod user_character;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ConnectionTrait, DbErr, DeleteResult, EntityTrait,
    IntoActiveModel,
};

/// Repository for managing user records in the database.
///
/// Provides CRUD operations for users including creation, retrieval, updates,
/// and deletion. Users are linked to their main character via foreign key.
pub struct UserRepository<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> UserRepository<'a, C> {
    /// Creates a new instance of UserRepository.
    ///
    /// Constructs a repository for managing user records in the database.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    ///
    /// # Returns
    /// - `UserRepository` - New repository instance
    pub fn new(db: &'a C) -> Self {
        Self { db }
    }

    /// Creates a new user with the specified main character.
    ///
    /// Inserts a new user record into the database with the given character ID set as
    /// their main character. The main character must exist in the database.
    ///
    /// # Arguments
    /// - `main_character_id` - Record ID of the character to set as main
    ///
    /// # Returns
    /// - `Ok(BifrostUser)` - The newly created user record
    /// - `Err(DbErr)` - Database operation failed or character ID doesn't exist
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

    /// Retrieves a user by ID along with their main character.
    ///
    /// Fetches a user record and joins it with their main character from the eve_character
    /// table. Returns None if the user doesn't exist.
    ///
    /// # Arguments
    /// - `user_id` - ID of the user to retrieve
    ///
    /// # Returns
    /// - `Ok(Some((BifrostUser, Some(EveCharacter))))` - User found with main character
    /// - `Ok(Some((BifrostUser, None)))` - User found but main character missing (should not happen with FK constraint)
    /// - `Ok(None)` - User does not exist
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_by_id(
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

    /// Updates a user's main character.
    ///
    /// Changes the main character for an existing user. The new main character must exist
    /// in the database. Returns None if the user doesn't exist.
    ///
    /// # Arguments
    /// - `user_id` - ID of the user to update
    /// - `new_main_character_id` - Record ID of the new main character
    ///
    /// # Returns
    /// - `Ok(Some(BifrostUser))` - User successfully updated
    /// - `Ok(None)` - User does not exist
    /// - `Err(DbErr)` - Database operation failed or new character ID doesn't exist
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

    /// Deletes a user by ID.
    ///
    /// Removes a user record from the database. Returns success regardless of whether
    /// the user existed. Check the rows_affected field to confirm deletion occurred.
    ///
    /// # Arguments
    /// - `user_id` - ID of the user to delete
    ///
    /// # Returns
    /// - `Ok(DeleteResult)` - Operation completed (check rows_affected: 1 if deleted, 0 if user didn't exist)
    /// - `Err(DbErr)` - Database operation failed
    pub async fn delete(&self, user_id: i32) -> Result<DeleteResult, DbErr> {
        entity::prelude::BifrostUser::delete_by_id(user_id)
            .exec(self.db)
            .await
    }
}

#[cfg(test)]
mod tests {

    mod create {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::user::UserRepository;

        /// Expect success when creating a new user
        #[tokio::test]
        async fn creates_user() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

            let user_repository = UserRepository::new(&test.state.db);
            let result = user_repository.create(character_model.id).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Error when setting user main character to character that does not exist in database
        #[tokio::test]
        async fn fails_for_nonexistent_main_character() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;

            let nonexistent_main_character_id = 2;
            let user_repository = UserRepository::new(&test.state.db);
            let result = user_repository.create(nonexistent_main_character_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod get_by_id {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::user::UserRepository;

        /// Expect Ok(Some(_)) when existing user is found
        #[tokio::test]
        async fn finds_existing_user() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let user_repo = UserRepository::new(&test.state.db);
            let result = user_repo.get_by_id(user_model.id).await;

            assert!(matches!(result, Ok(Some(_))));

            Ok(())
        }

        /// Expect Ok(None) when user is not found
        #[tokio::test]
        async fn returns_none_for_nonexistent_user() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;

            let nonexistent_user_id = 1;
            let user_repo = UserRepository::new(&test.state.db);
            let result = user_repo.get_by_id(nonexistent_user_id).await;

            assert!(matches!(result, Ok(None)));

            Ok(())
        }

        /// Expect Error when required database tables are not present
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;
            let user_repo = UserRepository::new(&test.state.db);

            let user_id = 1;
            let result = user_repo.get_by_id(user_id).await;
            assert!(result.is_err());

            Ok(())
        }
    }

    mod update {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::user::UserRepository;

        /// Expect Ok when updating user main character with valid character ID
        #[tokio::test]
        async fn updates_existing_user() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model_two = test.eve().insert_mock_character(2, 1, None, None).await?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let user_repo = UserRepository::new(&test.state.db);
            let result = user_repo
                .update(user_model.id, character_model_two.id)
                .await;

            assert!(matches!(result, Ok(Some(_))));
            let updated_user = result.unwrap().unwrap();
            assert_ne!(user_model.main_character_id, updated_user.main_character_id);

            Ok(())
        }

        /// Expect Ok(None) when attempting to update user ID that does not exist
        #[tokio::test]
        async fn returns_none_for_nonexistent_user() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let nonexistent_user_id = 1;
            let result = user_repo
                .update(nonexistent_user_id, character_model.id)
                .await;

            assert!(matches!(result, Ok(None)));

            Ok(())
        }

        /// Expect Error when attempting to update user main character with non existant character ID
        #[tokio::test]
        async fn fails_for_nonexistent_main_character() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let user_repo = UserRepository::new(&test.state.db);
            let result = user_repo
                .update(user_model.id, character_model.id + 1)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod delete {
        use bifrost_test_utils::prelude::*;
        use sea_orm::EntityTrait;

        use crate::server::data::user::UserRepository;

        /// Expect success when deleting user
        #[tokio::test]
        async fn deletes_existing_user() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
            let user_model = test.user().insert_user(character_model.id).await?;

            let user_repository = UserRepository::new(&test.state.db);
            let result = user_repository.delete(user_model.id).await;

            assert!(result.is_ok());
            let delete_result = result.unwrap();
            assert_eq!(delete_result.rows_affected, 1);
            // Ensure user has actually been deleted
            let user_exists = entity::prelude::BifrostUser::find_by_id(user_model.id)
                .one(&test.state.db)
                .await?;
            assert!(user_exists.is_none());

            Ok(())
        }

        /// Expect no rows to be affected when deleting user that does not exist
        #[tokio::test]
        async fn returns_no_rows_for_nonexistent_user() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let user_repository = UserRepository::new(&test.state.db);
            let result = user_repository.delete(user_model.id + 1).await;

            assert!(result.is_ok());
            let delete_result = result.unwrap();
            assert_eq!(delete_result.rows_affected, 0);

            Ok(())
        }

        /// Expect Error when database tables required don't exist
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            // Use test setup that doesn't create required tables, causing an error
            let test = test_setup_with_tables!()?;

            let user_id = 1;
            let user_repository = UserRepository::new(&test.state.db);
            let result = user_repository.delete(user_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
