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

    async fn setup() -> Result<(DatabaseConnection, entity::eve_character::Model), DbErr> {
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

        Ok((db, character))
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
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            let user = user_repository.create(character.id).await?;
            let result = user_character_repository
                .create(user.id, character.id, "owner hash".to_string())
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect error when creating user character linked to missing user
        #[tokio::test]
        async fn test_create_user_character_missing_user() -> Result<(), DbErr> {
            let (db, character) = setup().await?;
            let user_character_repository = UserCharacterRepository::new(&db);

            // Don't create a user first, this will cause a foreign key error
            let user_id = 1;
            let result = user_character_repository
                .create(user_id, character.id, "owner hash".to_string())
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
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            // Increment character ID to one that does not exist, causing a foreign key error
            let user = user_repository.create(character.id).await?;
            let result = user_character_repository
                .create(user.id, character.id + 1, "owner hash".to_string())
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
        use sea_orm::{DbErr, EntityTrait};

        use crate::server::{
            data::user::{
                user::UserRepository,
                user_character::{tests::setup, UserCharacterRepository},
            },
            util::test::setup::test_setup,
        };

        // Expect Some when character & character ownership entry is found
        #[tokio::test]
        async fn test_get_by_character_id_some_character_ownership() -> Result<(), DbErr> {
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            let user = user_repository.create(character.id).await?;
            let _ = user_character_repository
                .create(user.id, character.id, "owner hash".to_string())
                .await?;

            let result = user_character_repository
                .get_by_character_id(character.character_id)
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
        async fn test_get_by_character_id_some_character() -> Result<(), DbErr> {
            let (db, character) = setup().await?;
            let user_character_repository = UserCharacterRepository::new(&db);

            let result = user_character_repository
                .get_by_character_id(character.character_id)
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
        async fn test_get_by_character_id_none_character() -> Result<(), DbErr> {
            let (db, character) = setup().await?;
            let user_character_repository = UserCharacterRepository::new(&db);

            // Delete the character entry first
            entity::prelude::EveCharacter::delete_by_id(character.id)
                .exec(&db)
                .await?;

            let result = user_character_repository
                .get_by_character_id(character.character_id)
                .await;

            assert!(result.is_ok());
            let maybe_character = result.unwrap();

            assert!(maybe_character.is_none());

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

    mod get_many_by_user_id {
        use sea_orm::DbErr;

        use crate::server::{
            data::{
                eve::character::CharacterRepository,
                user::{
                    user::UserRepository,
                    user_character::{tests::setup, UserCharacterRepository},
                },
            },
            util::test::{eve::mock::mock_character, setup::test_setup},
        };

        /// Expect Ok with 2 owned character entries
        #[tokio::test]
        async fn test_get_many_by_user_id_multiple() -> Result<(), DbErr> {
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);
            let character_repository = CharacterRepository::new(&db);

            let user = user_repository.create(character.id).await?;
            let _ = user_character_repository
                .create(user.id, character.id, "owner hash".to_string())
                .await?;

            // Create an additional mock character
            let character_id = 2;
            let corporation_id = 1; // Use existing corporation from setup()
            let alliance_id = None;
            let faction_id = None;
            let second_character = mock_character(corporation_id, alliance_id, faction_id);
            let character = character_repository
                .create(character_id, second_character, 1, None)
                .await?;
            let _ = user_character_repository
                .create(user.id, character.id, "owner hash".to_string())
                .await?;

            let result = user_character_repository.get_many_by_user_id(user.id).await;

            assert!(result.is_ok());
            let ownership_entries = result.unwrap();

            assert_eq!(ownership_entries.len(), 2);

            Ok(())
        }

        /// Expect Ok with only 1 owned character entry
        #[tokio::test]
        async fn test_get_many_by_user_id_single() -> Result<(), DbErr> {
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            let user = user_repository.create(character.id).await?;
            let _ = user_character_repository
                .create(user.id, character.id, "owner hash".to_string())
                .await?;

            let result = user_character_repository.get_many_by_user_id(user.id).await;

            assert!(result.is_ok());
            let ownership_entries = result.unwrap();

            assert_eq!(ownership_entries.len(), 1);

            Ok(())
        }

        /// Expect Ok with empty Vec due to no owned characters
        #[tokio::test]
        async fn test_get_many_by_user_id_empty() -> Result<(), DbErr> {
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            let user = user_repository.create(character.id).await?;

            // Assign no ownerships to character, result will be empty

            let result = user_character_repository.get_many_by_user_id(user.id).await;

            assert!(result.is_ok());
            let ownership_entries = result.unwrap();

            assert!(ownership_entries.is_empty());

            Ok(())
        }

        /// Expect database error when required tables aren't present
        #[tokio::test]
        async fn test_get_many_by_user_id_error() -> Result<(), DbErr> {
            // Use test setup that doesn't create required tables, causing an error
            let test = test_setup().await;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);

            let user_id = 1;
            let result = user_character_repository.get_many_by_user_id(user_id).await;

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
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            let old_user = user_repository.create(character.id).await?;
            let user_character_entry = user_character_repository
                .create(old_user.id, character.id, "owner hash".to_string())
                .await?;

            let new_user = user_repository.create(character.id).await?;
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
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            // Try to update entry ID that doesn't exist
            // - Note: User has a main character set but no table doesn't actually show they own them
            let user_character_entry_id = 1;
            let new_user = user_repository.create(character.id).await?;
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
            let (db, character) = setup().await?;
            let user_repository = UserRepository::new(&db);
            let user_character_repository = UserCharacterRepository::new(&db);

            let old_user = user_repository.create(character.id).await?;
            let user_character_entry = user_character_repository
                .create(old_user.id, character.id, "owner hash".to_string())
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
