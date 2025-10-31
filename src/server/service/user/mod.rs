pub mod user_character;

use eve_esi::model::oauth2::EveJwtClaims;
use sea_orm::DatabaseConnection;

use crate::{
    model::user::{Character, UserDto},
    server::{
        data::user::{user_character::UserCharacterRepository, UserRepository},
        error::Error,
        service::{eve::character::CharacterService, user::user_character::UserCharacterService},
    },
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

    pub async fn get_or_create_user(&self, claims: EveJwtClaims) -> Result<i32, Error> {
        let user_repo = UserRepository::new(&self.db);
        let user_character_repo = UserCharacterRepository::new(&self.db);
        let character_service = CharacterService::new(&self.db, &self.esi_client);
        let user_character_service = UserCharacterService::new(&self.db, &self.esi_client);

        let character_id = claims.character_id()?;
        let character = match user_character_repo
            .get_by_character_id(character_id)
            .await?
        {
            Some((character, maybe_owner)) => {
                if let Some(ownership_entry) = maybe_owner {
                    // Validate whether or not character has been sold or transferred between accounts
                    if claims.owner == ownership_entry.owner_hash {
                        // User ownership hasn't changed, user still owns this character
                        return Ok(ownership_entry.id);
                    }

                    // Character has been sold or transferred, create a new user account
                    let new_user = user_repo.create(ownership_entry.character_id).await?;
                    user_character_service
                        .transfer_character(ownership_entry, new_user.id)
                        .await?;

                    return Ok(new_user.id);
                }

                // Character exists but not owned by any user, no need to create the character
                character
            }
            // Character not found in database, create the character
            None => character_service.create_character(character_id).await?,
        };

        // Create new user and link character to user
        let new_user = user_repo.create(character.id).await?;
        let _ = user_character_repo
            .create(new_user.id, character.id, claims.owner)
            .await?;

        Ok(new_user.id)
    }

    pub async fn get_user(&self, user_id: i32) -> Result<Option<UserDto>, Error> {
        let user_repo = UserRepository::new(&self.db);
        let user_character_repo = UserCharacterRepository::new(&self.db);

        match user_repo.get(user_id).await? {
            None => return Ok(None),
            Some((user, maybe_main_character)) => {
                let main_character = maybe_main_character.ok_or_else(|| {
                    // Should not occur due to foreign key constraint requiring main character to exist
                    Error::DbErr(sea_orm::DbErr::RecordNotFound(format!(
                        "Failed to find main character information for user ID {} with main character ID {}",
                        user.id, user.main_character_id
                    )))
                })?;

                let user_characters = user_character_repo
                    .get_owned_characters_by_user_id(user_id)
                    .await?;

                let characters: Vec<Character> = user_characters
                    .into_iter()
                    .filter(|c| c.character_id != main_character.character_id)
                    .map(|c| Character {
                        id: c.character_id,
                        name: c.name,
                    })
                    .collect();

                Ok(Some(UserDto {
                    id: user.id,
                    main_character: Character {
                        id: main_character.character_id,
                        name: main_character.name,
                    },
                    characters: characters,
                }))
            }
        }
    }

    /// Deletes the provided user ID
    ///
    /// # Warning
    /// This will error if you attempt to delete the user while they still have
    /// connected character ownerships, you must [`Self::transfer_character`] first
    /// to another user before deleting a user.
    pub async fn delete_user(&self, user_id: i32) -> Result<bool, Error> {
        let user_repo = UserRepository::new(&self.db);

        let delete_result = user_repo.delete(user_id).await?;

        Ok(delete_result.rows_affected == 1)
    }
}

#[cfg(test)]
mod tests {

    mod get_or_create_user {
        use bifrost_test_utils::prelude::*;
        use eve_esi::model::oauth2::EveJwtClaims;

        use crate::server::{
            data::user::user_character::UserCharacterRepository, error::Error,
            service::user::UserService,
        };

        /// Expect Ok when user associated with character is found
        #[tokio::test]
        async fn get_or_create_user_ok_found() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, user_character_model, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);
            claims.owner = user_character_model.owner_hash;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.get_or_create_user(claims).await;

            assert!(result.is_ok());
            let user_id = result.unwrap();
            assert_eq!(user_id, user_model.id);

            Ok(())
        }

        /// Expect Ok & character transfer if owner hash for character has changed, requiring a new user
        #[tokio::test]
        async fn get_or_create_user_ok_transfer_owner_hash_change() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (_, user_character_model, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();
            claims.owner = "different_owner_hash".to_string();

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.get_or_create_user(claims).await;

            assert!(result.is_ok());
            // Ensure character was actually transferred & new user created
            let user_character_result = user_character_repo
                .get_by_character_id(character_model.character_id)
                .await?;
            let (_, maybe_user_character_model) = user_character_result.unwrap();
            let updated_user_character_model = maybe_user_character_model.unwrap();

            assert_ne!(
                updated_user_character_model.user_id,
                user_character_model.user_id
            );

            Ok(())
        }

        /// Expect Ok when character is found but new user is created
        #[tokio::test]
        async fn get_or_create_user_ok_created_existing_character() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.get_or_create_user(claims).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok when new character & user is created
        #[tokio::test]
        async fn get_or_create_user_ok_created() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_id = 1;
            let character_endpoints =
                test.eve()
                    .with_character_endpoint(character_id, 1, None, None, 1);

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", character_id);

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.get_or_create_user(claims).await;

            assert!(result.is_ok());
            // Assert 1 request was made to each mock endpoint
            for endpoint in character_endpoints {
                endpoint.assert();
            }

            Ok(())
        }

        /// Expect Error when the required database tables haven't been created
        #[tokio::test]
        async fn get_or_create_user_err_missing_tables() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", 1);

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.get_or_create_user(claims).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        /// Expect Error when required ESI endpoints are unavailable
        #[tokio::test]
        async fn get_or_create_user_err_esi() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", 1);

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.get_or_create_user(claims).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }
    }

    mod get_user {
        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::user::UserService};

        /// Expect Ok with Some & no additional characters for user with only a main character linked
        #[tokio::test]
        async fn get_user_ok_some_only_main() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.get_user(user_model.id).await;
            assert!(result.is_ok());
            let maybe_user = result.unwrap();
            assert!(maybe_user.is_some());
            let user_info = maybe_user.unwrap();

            // Additional characters as only their main is linked should be empty
            assert!(user_info.characters.is_empty());

            Ok(())
        }

        /// Expect Ok with Some & 1 additional characters linked for user
        #[tokio::test]
        async fn get_user_ok_some_one_additional_character() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;
            let (_, _) = test
                .user()
                .insert_mock_character_for_user(user_model.id, 2, 1, None, None)
                .await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.get_user(user_model.id).await;
            assert!(result.is_ok());
            let maybe_user = result.unwrap();
            assert!(maybe_user.is_some());
            let user_info = maybe_user.unwrap();
            // Additional characters, which does not include main, should equal 1
            assert_eq!(user_info.characters.len(), 1);

            Ok(())
        }

        /// Expect Ok with None for user ID that does not exist
        #[tokio::test]
        async fn get_user_ok_none_non_existant_user() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;

            let non_existant_user_id = 1;
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.get_user(non_existant_user_id).await;

            assert!(result.is_ok());
            let maybe_user = result.unwrap();
            assert!(maybe_user.is_none());

            Ok(())
        }

        /// Expect Error when required tables are not present
        #[tokio::test]
        async fn get_user_err_missing_tables() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let non_existant_user_id = 1;
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.get_user(non_existant_user_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }
    }

    mod delete_user_tests {
        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::user::UserService};

        /// Expect Ok with true indicating user was deleted
        #[tokio::test]
        async fn delete_user_ok_true_deleted() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
            // We include the character ID as a main which must be set for every user, for this test
            // they don't actually need to own the character so no ownership record is set.
            let user_model = test.user().insert_user(character_model.id).await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.delete_user(user_model.id).await;

            assert!(result.is_ok());
            let user_deleted = result.unwrap();
            assert!(user_deleted);
            let maybe_user = user_service.get_user(user_model.id).await.unwrap();
            assert!(maybe_user.is_none());

            Ok(())
        }

        /// Expect Ok with false when trying to delete a user that does not exist
        #[tokio::test]
        async fn delete_user_ok_false_doesnt_exist() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;
            let non_existant_user_id = 1;
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.delete_user(non_existant_user_id).await;

            assert!(result.is_ok());
            let user_deleted = result.unwrap();
            assert!(!user_deleted);

            Ok(())
        }

        /// Expect Error when trying to delete user with existing character ownerships
        /// - This is due to a foreign key violation requiring a user ID to exist for
        ///   a character ownership entry.
        #[tokio::test]
        async fn delete_user_err_has_owned_characters() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);
            let result = user_service.delete_user(user_model.id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }
    }
}
