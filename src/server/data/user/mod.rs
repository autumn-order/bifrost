pub mod user_character;

use sea_orm::{
    ActiveModelTrait, ActiveValue, ConnectionTrait, DbErr, DeleteResult, EntityTrait,
    IntoActiveModel,
};

pub struct UserRepository<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> UserRepository<'a, C> {
    /// Creates a new instance of [`UserRepository`]
    pub fn new(db: &'a C) -> Self {
        Self { db }
    }

    /// Creates a new user
    pub async fn create(
        &self,
        main_character_id: i32,
    ) -> Result<entity::bifrost_user::Model, DbErr> {
        let user = entity::bifrost_user::ActiveModel {
            main_character_id: ActiveValue::Set(main_character_id),
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

    mod get {
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
            let result = user_repo.get(user_model.id).await;

            assert!(matches!(result, Ok(Some(_))));

            Ok(())
        }

        /// Expect Ok(None) when user is not found
        #[tokio::test]
        async fn returns_none_for_nonexistent_user() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;

            let nonexistent_user_id = 1;
            let user_repo = UserRepository::new(&test.state.db);
            let result = user_repo.get(nonexistent_user_id).await;

            assert!(matches!(result, Ok(None)));

            Ok(())
        }

        /// Expect Error when required database tables are not present
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;
            let user_repo = UserRepository::new(&test.state.db);

            let user_id = 1;
            let result = user_repo.get(user_id).await;
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
