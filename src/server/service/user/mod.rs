pub mod user_character;

use eve_esi::model::oauth2::EveJwtClaims;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::user::{user::UserRepository, user_character::UserCharacterRepository},
    error::Error,
    service::{eve::character::CharacterService, user::user_character::UserCharacterService},
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
    use sea_orm::{ConnectionTrait, DbBackend, DbErr, Schema};

    use crate::server::util::test::setup::{test_setup, TestSetup};

    async fn test_setup_module() -> Result<TestSetup, DbErr> {
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
        use eve_esi::model::oauth2::EveJwtClaims;

        use crate::server::{
            data::user::user_character::UserCharacterRepository,
            error::Error,
            service::user::{tests::test_setup_module, UserService},
            util::test::setup::{
                test_setup, test_setup_create_character, test_setup_create_character_endpoints,
                test_setup_create_corporation, test_setup_create_user_with_character,
            },
        };

        /// Expect success when user associated with character is found
        #[tokio::test]
        async fn test_get_or_create_user_found_user() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character =
                test_setup_create_character(&test, character_id, corporation.clone()).await?;
            let _ = test_setup_create_user_with_character(&test, character).await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            let result = user_service.get_or_create_user(claims).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok & character transfer if owner hash for character has changed, requiring a new user
        #[tokio::test]
        async fn test_get_or_create_user_transfer_success() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character = test_setup_create_character(&test, character_id, corporation).await?;
            let old_ownership_entry =
                test_setup_create_user_with_character(&test, character).await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();
            claims.owner = "different_owner_hash".to_string();

            let result = user_service.get_or_create_user(claims).await;

            assert!(result.is_ok());

            // Ensure character was actually transferred & new user created
            let ownership_entry = user_character_repo
                .get_by_character_id(character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();

            assert_ne!(character_ownership.user_id, old_ownership_entry.user_id);

            Ok(())
        }

        /// Expect success when character is found but new user is created
        #[tokio::test]
        async fn test_get_or_create_user_new_user() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let _ = test_setup_create_character(&test, character_id, corporation.clone()).await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            let result = user_service.get_or_create_user(claims).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect success when new character & user is created
        #[tokio::test]
        async fn test_get_or_create_user_new_character() -> Result<(), Error> {
            let mut test = test_setup_module().await?;
            let (mock_character_endpoint, mock_corporation_endpoint) =
                test_setup_create_character_endpoints(&mut test).await;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            let result = user_service.get_or_create_user(claims).await;

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

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            let result = user_service.get_or_create_user(claims).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        /// Expect Error when required ESI endpoints are unavailable
        #[tokio::test]
        async fn test_get_or_create_user_esi_error() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            // Don't create mock ESI endpoints, causing an ESI error

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            let result = user_service.get_or_create_user(claims).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }
    }

    mod delete_user_tests {
        use crate::server::{
            data::user::user::UserRepository,
            error::Error,
            service::user::{tests::test_setup_module, UserService},
            util::test::setup::{
                test_setup_create_character, test_setup_create_corporation,
                test_setup_create_user_with_character,
            },
        };

        /// Expect Ok with true indicating user was deleted
        #[tokio::test]
        async fn test_delete_user_success() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let user_repository = UserRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character = test_setup_create_character(&test, character_id, corporation).await?;

            let user = user_repository.create(character.id).await?;
            let result = user_service.delete_user(user.id).await;

            assert!(result.is_ok());
            let user_deleted = result.unwrap();

            assert!(user_deleted);

            Ok(())
        }

        /// Expect Ok with false when trying to delete a user that does not exist
        #[tokio::test]
        async fn test_delete_user_does_not_exist() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let non_existant_user_id = 1;
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
        async fn test_delete_user_owned_characters_error() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character = test_setup_create_character(&test, character_id, corporation).await?;
            let user = test_setup_create_user_with_character(&test, character).await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let result = user_service.delete_user(user.id).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
