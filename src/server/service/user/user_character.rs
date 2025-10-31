use eve_esi::model::oauth2::EveJwtClaims;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::user::{user_character::UserCharacterRepository, UserRepository},
    error::Error,
    service::{eve::character::CharacterService, user::UserService},
};

pub struct UserCharacterService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> UserCharacterService<'a> {
    /// Creates a new instance of [`UserService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
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
        let user_service = UserService::new(&self.db, &self.esi_client);

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
            let _ = user_service.delete_user(ownership_entry.user_id).await?;
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
}

#[cfg(test)]
mod tests {

    mod link_character {
        use bifrost_test_utils::prelude::*;
        use eve_esi::model::oauth2::EveJwtClaims;

        use crate::server::{
            data::user::user_character::UserCharacterRepository, error::Error,
            service::user::user_character::UserCharacterService,
        };

        /// Expect no link created when finding character owned by provided user ID
        #[tokio::test]
        async fn test_link_character_owned_success() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, user_character_model, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);
            claims.owner = user_character_model.owner_hash;

            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service
                .link_character(user_model.id, claims)
                .await;

            assert!(result.is_ok());
            let link_created = result.unwrap();
            assert!(!link_created);

            Ok(())
        }

        /// Expect Ok & character transfer if owner hash hasn't changed but user ID is different
        #[tokio::test]
        async fn test_link_character_owned_different_user_transfer() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (_, user_character_model, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;
            let new_user_model = test
                .user()
                .insert_user(user_character_model.character_id)
                .await?;

            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);
            claims.owner = user_character_model.owner_hash;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service
                .link_character(new_user_model.id, claims)
                .await;

            assert!(result.is_ok());
            let link_created = result.unwrap();
            assert!(link_created);

            // Ensure character was actually transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_model.character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();
            assert_eq!(character_ownership.user_id, new_user_model.id);

            Ok(())
        }

        /// Expect Ok & character transfer if ownerhash for character has changed, requiring a new user
        #[tokio::test]
        async fn test_link_character_owned_transfer_success() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (_, user_character_model, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;
            let new_user_model = test
                .user()
                .insert_user(user_character_model.character_id)
                .await?;

            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);
            claims.owner = format!("different_{}", user_character_model.owner_hash);

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service
                .link_character(new_user_model.id, claims)
                .await;

            assert!(result.is_ok());
            let link_created = result.unwrap();
            assert!(link_created);

            // Ensure character was actually transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_model.character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();
            assert_eq!(character_ownership.user_id, new_user_model.id);

            Ok(())
        }

        /// Expect link created when character is created but not owned and linked to provided user ID
        #[tokio::test]
        async fn test_link_character_not_owned_success() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
            // Character is set as main but there isn't actually an ownership record set
            let user_model = test.user().insert_user(character_model.id).await?;

            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service
                .link_character(user_model.id, claims)
                .await;

            assert!(result.is_ok());
            let link_created = result.unwrap();
            assert!(link_created);

            // Ensure character was actually linked
            let ownership_entry = user_character_repo
                .get_by_character_id(character_model.character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();
            assert_eq!(character_ownership.user_id, user_model.id);

            Ok(())
        }

        /// Expect link created when creating a new character and linking to provided user ID
        #[tokio::test]
        async fn test_link_character_create_character_success() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;
            let second_character_id = 2;
            let endpoints =
                test.eve()
                    .with_character_endpoint(second_character_id, 2, None, None, 1);

            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", second_character_id);

            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service
                .link_character(user_model.id, claims)
                .await;

            assert!(result.is_ok());
            let link_created = result.unwrap();
            assert!(link_created);

            // Assert 1 request was made to each mock endpoint
            for endpoint in endpoints {
                endpoint.assert()
            }

            Ok(())
        }

        /// Expect database Error when user ID provided does not exist in database
        #[tokio::test]
        async fn test_link_character_user_id_foreign_key_database_error() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);

            let non_existant_id = 1;
            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service
                .link_character(non_existant_id, claims)
                .await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        /// Expect ESI error when endpoints required to create a character are not available
        #[tokio::test]
        async fn test_link_character_create_character_esi_error() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;

            let character_id = 1;
            let mut claims = EveJwtClaims::mock();
            claims.sub = format!("CHARACTER:EVE:{}", character_id);

            let user_id = 1;
            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service.link_character(user_id, claims).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }
    }

    mod transfer_character_tests {
        use bifrost_test_utils::prelude::*;

        use crate::server::{
            data::user::{user_character::UserCharacterRepository, UserRepository},
            service::user::user_character::UserCharacterService,
        };

        /// Expect Ok with user deletion when last character is transferred
        #[tokio::test]
        async fn test_transfer_character_with_deletion_success() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (_, user_character_model, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;
            // Character is set as main but there isn't actually an ownership record set so it will transfer
            let new_user_model = test.user().insert_user(character_model.id).await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service
                .transfer_character(user_character_model, new_user_model.id)
                .await;

            assert!(result.is_ok());
            let previous_user_deleted = result.unwrap();
            assert!(previous_user_deleted);

            // Ensure character was actually transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_model.character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();
            assert_eq!(character_ownership.user_id, new_user_model.id);

            Ok(())
        }

        /// Expect Ok with no user deletion when character is transferred from user with multiple characters
        /// - No main change
        #[tokio::test]
        async fn transfer_character_without_deletion() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;
            let (second_user_character_model, character_model) = test
                .user()
                .insert_mock_character_owned_by_user(user_model.id, 2, 1, None, None)
                .await?;
            // Character is set as main but there isn't actually an ownership record set so it will transfer
            let new_user_model = test.user().insert_user(character_model.id).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service
                .transfer_character(second_user_character_model, new_user_model.id)
                .await;

            assert!(result.is_ok());
            let previous_user_deleted = result.unwrap();
            assert!(!previous_user_deleted);

            // Ensure character was actually transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_model.character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();
            assert_eq!(character_ownership.user_id, new_user_model.id);

            // Ensure main character was not changed since it wasn't transferred
            let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
            assert_eq!(
                user_model.main_character_id,
                updated_user_model.main_character_id
            );

            Ok(())
        }

        /// Expect Ok with no user deletion when character is transferred from user with multiple characters
        /// - change main
        #[tokio::test]
        async fn transfer_character_with_change_main() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, main_user_character_model, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;
            let (_, _) = test
                .user()
                .insert_mock_character_owned_by_user(user_model.id, 2, 1, None, None)
                .await?;
            // Character is set as main but there isn't actually an ownership record set so it will transfer
            let new_user_model = test.user().insert_user(character_model.id).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service
                .transfer_character(main_user_character_model, new_user_model.id)
                .await;

            assert!(result.is_ok());
            let previous_user_deleted = result.unwrap();
            assert!(!previous_user_deleted);

            // Ensure character was actually transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_model.character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let character_ownership = maybe_ownership.unwrap();
            assert_eq!(character_ownership.user_id, new_user_model.id);

            // Ensure main character was changed since it was transferred
            let (updated_user_model, _) = user_repo.get(user_model.id).await?.unwrap();
            assert_ne!(
                user_model.main_character_id,
                updated_user_model.main_character_id
            );

            Ok(())
        }

        /// Expect Error transferring character to user that does not exist
        #[tokio::test]
        async fn test_transfer_character_error() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, user_character_model, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let user_character_service =
                UserCharacterService::new(&test.state.db, &test.state.esi_client);
            let result = user_character_service
                .transfer_character(user_character_model.clone(), user_model.id + 1)
                .await;

            assert!(result.is_err());

            // Ensure character was not transferred
            let ownership_entry = user_character_repo
                .get_by_character_id(character_model.character_id)
                .await?;
            let (_, maybe_ownership) = ownership_entry.unwrap();
            let latest_character_ownership = maybe_ownership.unwrap();
            assert_eq!(
                latest_character_ownership.user_id,
                user_character_model.user_id
            );

            Ok(())
        }
    }
}
