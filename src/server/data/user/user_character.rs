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
    /// - `owner_hash` (`String`): A string representing the ownership of the character
    pub async fn create(
        &self,
        user_id: i32,
        character_id: i32,
        owner_hash: String,
    ) -> Result<entity::bifrost_user_character::Model, DbErr> {
        let user_character = entity::bifrost_user_character::ActiveModel {
            user_id: ActiveValue::Set(user_id),
            character_id: ActiveValue::Set(character_id),
            owner_hash: ActiveValue::Set(owner_hash),
            created_at: ActiveValue::Set(Utc::now().naive_utc()),
            updated_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        user_character.insert(self.db).await
    }

    /// Get a user character entry using their EVE Online character ID
    pub async fn get_by_character_id(
        &self,
        character_id: i64,
    ) -> Result<
        Option<(
            entity::eve_character::Model,
            Option<entity::bifrost_user_character::Model>,
        )>,
        DbErr,
    > {
        entity::prelude::EveCharacter::find()
            .filter(entity::eve_character::Column::CharacterId.eq(character_id))
            .find_also_related(entity::bifrost_user_character::Entity)
            .one(self.db)
            .await
    }

    /// Gets all character ownership entries for the provided user ID
    pub async fn get_many_by_user_id(
        &self,
        user_id: i32,
    ) -> Result<Vec<entity::bifrost_user_character::Model>, DbErr> {
        entity::prelude::BifrostUserCharacter::find()
            .filter(entity::bifrost_user_character::Column::UserId.eq(user_id))
            .all(self.db)
            .await
    }

    /// Gets character information for all characters owned by the user
    pub async fn get_owned_characters_by_user_id(
        &self,
        user_id: i32,
    ) -> Result<Vec<entity::eve_character::Model>, sea_orm::DbErr> {
        let joined: Vec<(
            entity::bifrost_user_character::Model,
            Option<entity::eve_character::Model>,
        )> = entity::prelude::BifrostUserCharacter::find()
            .filter(entity::bifrost_user_character::Column::UserId.eq(user_id))
            .find_also_related(entity::prelude::EveCharacter)
            .all(self.db)
            .await?;
        let characters = joined
            .into_iter()
            .filter_map(|(_, maybe_char)| maybe_char)
            .collect();
        Ok(characters)
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

    mod create {
        use bifrost_test_utils::prelude::*;
        use sea_orm::{DbErr, RuntimeErr};

        use crate::server::data::user::user_character::UserCharacterRepository;

        /// Expect success when creating user character linked to existing user and character
        #[tokio::test]
        async fn test_create_user_character_success() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.insert_mock_character(1, 1, None, None).await?;
            let user_model = test.user().insert_user(character_model.id).await?;

            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .create(user_model.id, character_model.id, "owner_hash".to_string())
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect error when creating user character linked to missing user
        #[tokio::test]
        async fn test_create_user_character_missing_user() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;
            let character_model = test.insert_mock_character(1, 1, None, None).await?;

            // Don't create a user first, this will cause a foreign key error
            let user_id = 1;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .create(user_id, character_model.id, "owner_hash".to_string())
                .await;

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

        /// Expect error when creating user character linked to missing character
        #[tokio::test]
        async fn test_create_user_character_missing_character() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.insert_mock_character(1, 1, None, None).await?;
            let user_model = test.user().insert_user(character_model.id).await?;

            // Increment character ID to one that does not exist, causing a foreign key error
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .create(
                    user_model.id,
                    character_model.id + 1,
                    "owner hash".to_string(),
                )
                .await;

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

    mod get_by_character_id {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::user::user_character::UserCharacterRepository;

        // Expect Some when character & character ownership entry is found
        #[tokio::test]
        async fn test_get_by_character_id_some_character_ownership() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (_, _, character_model) = test
                .user()
                .insert_mock_user_with_character(1, 1, None, None)
                .await?;

            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_by_character_id(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let maybe_character = result.unwrap();
            assert!(maybe_character.is_some());
            let (_, maybe_owner) = maybe_character.unwrap();
            assert!(maybe_owner.is_some());

            Ok(())
        }

        // Expect Some when character entry is found but no character ownership entry
        #[tokio::test]
        async fn test_get_by_character_id_some_character() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;
            let character_model = test.insert_mock_character(1, 1, None, None).await?;

            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_by_character_id(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let maybe_character = result.unwrap();
            assert!(maybe_character.is_some());
            let (_, maybe_owner) = maybe_character.unwrap();
            assert!(maybe_owner.is_none());

            Ok(())
        }

        // Expect None when character is not found
        #[tokio::test]
        async fn test_get_by_character_id_none_character() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;

            let non_existant_character_id = 1;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_by_character_id(non_existant_character_id)
                .await;

            assert!(result.is_ok());
            let maybe_character = result.unwrap();
            assert!(maybe_character.is_none());

            Ok(())
        }

        // Expect Error when required database tables are not present
        #[tokio::test]
        async fn test_get_by_character_id_error() -> Result<(), TestError> {
            // Use test setup that does not create required tables, causing a database error
            let test = test_setup_with_tables!()?;

            let character_id = 1;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_by_character_id(character_id)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod get_many_by_user_id {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::user::user_character::UserCharacterRepository;

        /// Expect Ok with 2 owned character entries
        #[tokio::test]
        async fn test_get_many_by_user_id_multiple() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_mock_user_with_character(1, 1, None, None)
                .await?;
            let (_, _) = test
                .user()
                .insert_mock_character_owned_by_user(user_model.id, 2, 1, None, None)
                .await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo.get_many_by_user_id(user_model.id).await;

            assert!(result.is_ok());
            let ownership_entries = result.unwrap();
            assert_eq!(ownership_entries.len(), 2);

            Ok(())
        }

        /// Expect Ok with only 1 owned character entry
        #[tokio::test]
        async fn test_get_many_by_user_id_single() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_mock_user_with_character(1, 1, None, None)
                .await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo.get_many_by_user_id(user_model.id).await;

            assert!(result.is_ok());
            let ownership_entries = result.unwrap();
            assert_eq!(ownership_entries.len(), 1);

            Ok(())
        }

        /// Expect Ok with empty Vec due to no owned characters
        #[tokio::test]
        async fn test_get_many_by_user_id_empty() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.insert_mock_character(1, 1, None, None).await?;
            // Character is set as main but it is not actually owned due to no ownership entry
            let user_model = test.user().insert_user(character_model.id).await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo.get_many_by_user_id(user_model.id).await;

            assert!(result.is_ok());
            let ownership_entries = result.unwrap();
            assert!(ownership_entries.is_empty());

            Ok(())
        }

        /// Expect database error when required tables aren't present
        #[tokio::test]
        async fn test_get_many_by_user_id_error() -> Result<(), TestError> {
            // Use test setup that doesn't create required tables, causing an error
            let test = test_setup_with_tables!()?;

            let user_id = 1;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository.get_many_by_user_id(user_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod get_owned_characters_by_user_id {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::user::user_character::UserCharacterRepository;

        /// Expect Ok with Vec length of 1 when requesting valid user ID
        #[tokio::test]
        async fn returns_owned_characters_for_user() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_mock_user_with_character(1, 1, None, None)
                .await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .get_owned_characters_by_user_id(user_model.id)
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 1);

            Ok(())
        }

        /// Expect Ok with empty Vec when requesting a non existant user ID
        #[tokio::test]
        async fn returns_empty_for_nonexistent_user() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;

            let non_existant_user_id = 1;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_owned_characters_by_user_id(non_existant_user_id)
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 0);

            Ok(())
        }

        /// Expect Error when required database tables do not exist
        #[tokio::test]
        async fn error_when_tables_missing() -> Result<(), TestError> {
            // Use test setup that doesn't setup required tables, causing a database error
            let test = test_setup_with_tables!()?;

            let non_existant_user_id = 1;
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .get_owned_characters_by_user_id(non_existant_user_id)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod update {
        use bifrost_test_utils::prelude::*;
        use sea_orm::{DbErr, RuntimeErr};

        use crate::server::data::user::user_character::UserCharacterRepository;

        /// Expect Some when user character update is successful
        #[tokio::test]
        async fn test_update_user_character_some() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (_, user_one_character_model, _) = test
                .user()
                .insert_mock_user_with_character(1, 1, None, None)
                .await?;
            let (user_two_model, _, _) = test
                .user()
                .insert_mock_user_with_character(2, 1, None, None)
                .await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .update(user_one_character_model.id, user_two_model.id)
                .await;

            assert!(result.is_ok());
            let result_option = result.unwrap();
            assert!(result_option.is_some());

            Ok(())
        }

        /// Expect None when user character entry is not found
        #[tokio::test]
        async fn test_update_user_character_none() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.insert_mock_character(1, 1, None, None).await?;
            // Set character ID as main but don't actually set any ownership records
            let user_model = test.user().insert_user(character_model.id).await?;

            let non_existant_id = 1;
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .update(non_existant_id, user_model.id)
                .await;

            assert!(result.is_ok());
            let result_option = result.unwrap();
            assert!(result_option.is_none());

            Ok(())
        }

        /// Expect Error when updating user character entry to user that doesn't exist
        #[tokio::test]
        async fn test_update_user_character_error() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, user_character_model, _) = test
                .user()
                .insert_mock_user_with_character(1, 1, None, None)
                .await?;

            // Try to update entry to new_user_id that doesn't exist
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .update(user_character_model.id, user_model.id + 1)
                .await;

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
