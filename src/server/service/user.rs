use eve_esi::model::oauth2::EveJwtClaims;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::user::{user::UserRepository, user_character::UserCharacterRepository},
    error::Error,
    service::eve::character::CharacterService,
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
                    self.transfer_character(ownership_entry, new_user.id)
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

    /// Links or transfers character to provided user ID
    ///
    /// # Behavior
    /// - If the character is already linked to the provided user (owner hash matches `claims.owner` &
    ///   user ID matches the logged in user ID), no action is taken and the method returns `Ok(false)`.
    /// - If the character is linked to a different owner hash or user ID, the method returns `Ok(true)`
    ///   to indicate a transfer to the provided user ID
    /// - If the character exists but has no owner, a link is created that associates the
    ///   character with the provided `user_id` and owner hash, and the method returns `Ok(true)`.
    /// - If the character does not exist, it is fetched/created via ESI and then linked to `user_id`,
    ///   and the method returns `Ok(true)`
    ///
    /// # Arguments
    /// - `user_id` (`i32`): The ID of the user to link or transfer the character to. If the user does not exist
    ///   in the database then a database error will be returned (foreign-key constraint)
    /// - `claims` ([`EveJwtClaims`]): JWT claims returned by EVE OAuth2. Contains the character ID
    ///   (`claims.character_id()`) and an owner hash (`claims.owner`) used to determine current ownership.
    ///
    /// # Returns
    /// - `Ok(false)`: No link was created because the character is already linked to the `claims.owner`
    /// - `Ok(true)`: A link was created or the character was transferred to the provided user ID
    /// - `Err(Error::DbErr(_))`: Database error such as a foreign key violation due to invalid `user_id`
    /// - `Err(Error::EsiError(_))`: Error when making an ESI request for character information or parsing
    ///   character ID from claims (e.g. `claims.character_id()`)
    pub async fn link_character(&self, user_id: i32, claims: EveJwtClaims) -> Result<bool, Error> {
        let user_character_repo = UserCharacterRepository::new(&self.db);
        let character_service = CharacterService::new(&self.db, &self.esi_client);

        let character_id = claims.character_id()?;

        // If the character exists, check ownership
        if let Some((character, maybe_ownership)) = user_character_repo
            .get_by_character_id(character_id)
            .await?
        {
            if let Some(ownership) = maybe_ownership {
                if ownership.owner_hash == claims.owner && user_id == ownership.user_id {
                    // already linked to this owner -> nothing to do
                    return Ok(false);
                }

                // existing character linked to different owner -> transfer
                self.transfer_character(ownership, user_id).await?;

                return Ok(true);
            }

            // existing character but no owner -> create link
            user_character_repo
                .create(user_id, character.id, claims.owner)
                .await?;

            return Ok(true);
        }

        // character doesn't exist -> create, then link
        let character = character_service.create_character(character_id).await?;
        user_character_repo
            .create(user_id, character.id, claims.owner)
            .await?;

        Ok(true)
    }

    /// Transfers a character from one user to another
    ///
    /// # Behavior
    /// - If this character is the only remaining character for the user,
    ///   the user will then be deleted as they have no way to login.
    pub async fn transfer_character(
        &self,
        ownership_entry: entity::bifrost_user_character::Model,
        new_user_id: i32,
    ) -> Result<bool, Error> {
        let user_repo = UserRepository::new(&self.db);
        let user_character_repo = UserCharacterRepository::new(&self.db);

        let (old_user, _) = match user_repo.get(ownership_entry.user_id).await? {
            Some(user) => user,
            None => {
                // This shouldn't occur due to DB foreign key constraints requiring a valid user ID
                return Err(Error::DbErr(sea_orm::DbErr::RecordNotFound(format!(
                    "User not found for user character ownership entry ID {}",
                    ownership_entry.user_id
                ))));
            }
        };

        let ownership_entries = user_character_repo
            .get_many_by_user_id(ownership_entry.user_id)
            .await?;

        user_character_repo
            .update(ownership_entry.id, new_user_id)
            .await?;

        // If this was the last character for the user, delete them
        if ownership_entries.len() == 1 {
            let _ = self.delete_user(ownership_entry.user_id).await?;
            return Ok(true);
        }

        // If the user's main character was transferred, change main to oldest linked character
        if ownership_entry.character_id == old_user.main_character_id {
            if let Some(character) = ownership_entries
                .iter()
                .filter(|e| e.character_id != old_user.main_character_id)
                .min_by_key(|e| e.created_at)
            {
                if user_repo
                    .update(old_user.id, character.character_id)
                    .await?
                    .is_none()
                {
                    // This shouldn't occur unless the user were to be deleted while we are trying to update them
                    return Err(Error::DbErr(sea_orm::DbErr::RecordNotFound(format!(
                        "User with ID not found {}",
                        old_user.id
                    ))));
                }
            } else {
                // This shouldn't occur as we delete the user if there is no alternative characters
                return Err(Error::DbErr(sea_orm::DbErr::RecordNotFound(format!(
                    "No alternative character for user {} after removing main character ID {}",
                    old_user.id, old_user.main_character_id
                ))));
            }
        }

        Ok(false)
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

    mod link_character_tests {
        use eve_esi::model::oauth2::EveJwtClaims;

        use crate::server::{
            data::user::{user::UserRepository, user_character::UserCharacterRepository},
            error::Error,
            service::user::{tests::test_setup_module, UserService},
            util::test::setup::{
                test_setup_create_character, test_setup_create_character_endpoints,
                test_setup_create_corporation, test_setup_create_user_with_character,
            },
        };

        /// Expect no link created when finding character owned by provided user ID
        #[tokio::test]
        async fn test_link_character_owned_success() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character = test_setup_create_character(&test, character_id, corporation).await?;
            let character_ownership =
                test_setup_create_user_with_character(&test, character).await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();
            claims.owner = "test_owner_hash".to_string();

            let result = user_service
                .link_character(character_ownership.user_id, claims)
                .await;

            assert!(result.is_ok());
            let link_created = result.unwrap();

            assert!(!link_created);

            Ok(())
        }

        /// Expect Ok & character transfer if owner hash hasn't changed but user ID is different
        #[tokio::test]
        async fn test_link_character_owned_different_user_transfer() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character = test_setup_create_character(&test, character_id, corporation).await?;
            let character_main_id = character.id;
            let _ = test_setup_create_user_with_character(&test, character).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();
            claims.owner = "test_owner_hash".to_string();

            let new_user = user_repo.create(character_main_id).await?;
            let result = user_service.link_character(new_user.id, claims).await;

            assert!(result.is_ok());
            let link_created = result.unwrap();

            assert!(link_created);

            // Ensure character was actually transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();

            assert_eq!(character_ownership.user_id, new_user.id);

            Ok(())
        }

        /// Expect Ok & character transfer if ownerhash for character has changed, requiring a new user
        #[tokio::test]
        async fn test_link_character_owned_transfer_success() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character = test_setup_create_character(&test, character_id, corporation).await?;
            let character_main_id = character.id;
            let _ = test_setup_create_user_with_character(&test, character).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();
            claims.owner = "different_owner_hash".to_string();

            let new_user = user_repo.create(character_main_id).await?;
            let result = user_service.link_character(new_user.id, claims).await;

            assert!(result.is_ok());
            let link_created = result.unwrap();

            assert!(link_created);

            // Ensure character was actually transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();

            assert_eq!(character_ownership.user_id, new_user.id);

            Ok(())
        }

        /// Expect link created when character is created but not owned and linked to provided user ID
        #[tokio::test]
        async fn test_link_character_not_owned_success() -> Result<(), Error> {
            let test = test_setup_module().await?;
            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character = test_setup_create_character(&test, character_id, corporation).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            // Note: character is set as main character for user but they aren't actually set as owned
            let user = user_repo.create(character.id).await?;
            let result = user_service.link_character(user.id, claims).await;

            assert!(result.is_ok());
            let link_created = result.unwrap();

            assert!(link_created);

            Ok(())
        }

        /// Expect link created when creating a new character and linking to provided user ID
        #[tokio::test]
        async fn test_link_character_create_character_success() -> Result<(), Error> {
            let mut test = test_setup_module().await?;
            let (mock_character_endpoint, mock_corporation_endpoint) =
                test_setup_create_character_endpoints(&mut test).await;

            let user_repo = UserRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            // Add existing character to represent user's main character
            let character_id = 2;
            let corporation_id = 2;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character = test_setup_create_character(&test, character_id, corporation).await?;

            let user = user_repo.create(character.id).await?;
            let result = user_service.link_character(user.id, claims).await;

            assert!(result.is_ok());
            let link_created = result.unwrap();

            assert!(link_created);

            // Assert 1 request was made to each mock endpoint
            mock_corporation_endpoint.assert();
            mock_character_endpoint.assert();

            Ok(())
        }

        /// Expect database Error when user ID provided does not exist in database
        #[tokio::test]
        async fn test_link_character_user_id_foreign_key_database_error() -> Result<(), Error> {
            let test = test_setup_module().await?;
            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let _ = test_setup_create_character(&test, character_id, corporation).await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            let user_id = 1;
            let result = user_service.link_character(user_id, claims).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        /// Expect ESI error when endpoints required to create a character are not available
        #[tokio::test]
        async fn test_link_character_create_character_esi_error() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            let user_id = 1;
            let result = user_service.link_character(user_id, claims).await;

            assert!(result.is_err());
            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }
    }

    mod transfer_character_tests {
        use crate::server::{
            data::user::{user::UserRepository, user_character::UserCharacterRepository},
            error::Error,
            service::user::{tests::test_setup_module, UserService},
            util::test::setup::{
                test_setup_create_character, test_setup_create_corporation,
                test_setup_create_user_with_character,
            },
        };

        /// Expect Ok with user deletion when last character is transferred
        #[tokio::test]
        async fn test_transfer_character_with_deletion_success() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character = test_setup_create_character(&test, character_id, corporation).await?;
            let character_main_id = character.id;
            let character_ownership =
                test_setup_create_user_with_character(&test, character).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            // We'll add character as main just to satisfy the foreign key relation, doesn't matter for this test
            let new_user = user_repo.create(character_main_id).await?;
            let result = user_service
                .transfer_character(character_ownership, new_user.id)
                .await;

            assert!(result.is_ok());
            let previous_user_deleted = result.unwrap();

            assert!(previous_user_deleted);

            // Ensure character was actually transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();

            assert_eq!(character_ownership.user_id, new_user.id);

            Ok(())
        }

        /// Expect Ok with no user deletion when character is transferred from user with multiple characters
        #[tokio::test]
        async fn transfer_character_without_deletion() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character =
                test_setup_create_character(&test, character_id, corporation.clone()).await?;
            let character_ownership =
                test_setup_create_user_with_character(&test, character).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            // Get old user for later main character transfer check
            let (old_user, _) = user_repo.get(character_ownership.user_id).await?.unwrap();

            // Add an additional character to the old user so they don't get deleted
            let second_character_id = 2;
            let second_character =
                test_setup_create_character(&test, second_character_id, corporation).await?;
            let _ = user_character_repo
                .create(
                    character_ownership.user_id,
                    second_character.id,
                    "owner_hash".to_string(),
                )
                .await?;

            // We'll add second character as main just to satisfy the foreign key relation, doesn't matter for this test
            let new_user = user_repo.create(second_character.id).await?;
            let result = user_service
                .transfer_character(character_ownership, new_user.id)
                .await;

            assert!(result.is_ok());
            let previous_user_deleted = result.unwrap();

            assert!(!previous_user_deleted);

            // Ensure character was actually transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();

            assert_eq!(character_ownership.user_id, new_user.id);

            // Ensure main character was changed since it was transferred
            let (updated_old_user, _) = user_repo.get(character_ownership.user_id).await?.unwrap();
            assert_ne!(
                old_user.main_character_id,
                updated_old_user.main_character_id
            );

            Ok(())
        }

        /// Expect Ok with no user deletion when character is transferred from user with multiple characters
        #[tokio::test]
        async fn transfer_character_with_change_main() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let main_character =
                test_setup_create_character(&test, character_id, corporation.clone()).await?;
            let main_character_ownership =
                test_setup_create_user_with_character(&test, main_character).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            // Get old user for later main character transfer check
            let (old_user, _) = user_repo
                .get(main_character_ownership.user_id)
                .await?
                .unwrap();

            // Add an additional character to the old user so they don't get deleted
            let second_character_id = 2;
            let second_character =
                test_setup_create_character(&test, second_character_id, corporation).await?;
            let second_character_ownership = user_character_repo
                .create(
                    main_character_ownership.user_id,
                    second_character.id,
                    "owner_hash".to_string(),
                )
                .await?;

            // We'll add second character as main just to satisfy the foreign key relation, doesn't matter for this test
            let new_user = user_repo.create(second_character.id).await?;
            let _ = user_service
                .transfer_character(main_character_ownership, new_user.id)
                .await?;

            // Ensure main character was changed since it was transferred
            let (updated_old_user, _) = user_repo
                .get(second_character_ownership.user_id)
                .await?
                .unwrap();

            assert_ne!(
                old_user.main_character_id,
                updated_old_user.main_character_id
            );

            Ok(())
        }

        /// Expect Ok with no user deletion when character is transferred from user with multiple characters
        #[tokio::test]
        async fn transfer_character_without_change_main() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let main_character =
                test_setup_create_character(&test, character_id, corporation.clone()).await?;
            let main_character_ownership =
                test_setup_create_user_with_character(&test, main_character).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            // Get old user for later main character transfer check
            let (old_user, _) = user_repo
                .get(main_character_ownership.user_id)
                .await?
                .unwrap();

            // Add an additional character to the old user so they don't get deleted
            let second_character_id = 2;
            let second_character =
                test_setup_create_character(&test, second_character_id, corporation).await?;
            let second_character_ownership = user_character_repo
                .create(
                    main_character_ownership.user_id,
                    second_character.id,
                    "owner_hash".to_string(),
                )
                .await?;

            // We'll add second character as main just to satisfy the foreign key relation, doesn't matter for this test
            let new_user = user_repo.create(second_character.id).await?;
            let _ = user_service
                .transfer_character(second_character_ownership, new_user.id)
                .await?;

            // Ensure main character was not changed since the main itself wasn't transferred
            let (updated_old_user, _) = user_repo
                .get(main_character_ownership.user_id)
                .await?
                .unwrap();
            assert_eq!(
                old_user.main_character_id,
                updated_old_user.main_character_id
            );

            Ok(())
        }

        /// Expect Error transferring character to user that does not exist
        #[tokio::test]
        async fn test_transfer_character_error() -> Result<(), Error> {
            let test = test_setup_module().await?;

            let character_id = 1;
            let corporation_id = 1;
            let corporation = test_setup_create_corporation(&test, corporation_id).await?;
            let character = test_setup_create_character(&test, character_id, corporation).await?;
            let character_ownership =
                test_setup_create_user_with_character(&test, character).await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let non_existant_user_id = 2;
            let result = user_service
                .transfer_character(character_ownership.clone(), non_existant_user_id)
                .await;

            assert!(result.is_err());

            // Ensure character was not transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let latest_character_ownership = maybe_ownership.unwrap();

            assert_eq!(
                latest_character_ownership.user_id,
                character_ownership.user_id
            );

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
