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
                if let Some(character_owner) = maybe_owner {
                    return Ok(character_owner.id);
                }

                character
            }
            None => character_service.create_character(character_id).await?,
        };

        let new_user = user_repo.create().await?;
        let _ = user_character_repo
            .create(new_user.id, character.id, claims.owner)
            .await?;

        Ok(new_user.id)
    }

    /// Links or transfers character to provided user ID
    ///
    /// # Behavior
    /// - If the character is already linked to the provided user (owner hash matches `claims.owner`),
    ///   no action is taken and the method returns `Ok(false)`.
    /// - If the character is linked to a different owner hash, the method currently returns `Ok(true)`
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
        if let Some((character, maybe_owner)) = user_character_repo
            .get_by_character_id(character_id)
            .await?
        {
            if let Some(owner) = maybe_owner {
                if owner.owner_hash == claims.owner {
                    // already linked to this owner -> nothing to do
                    return Ok(false);
                }

                // TODO: existing character linked to different owner -> transfer
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
            error::Error,
            service::user::{tests::test_setup_module, UserService},
            util::test::setup::{
                test_setup, test_setup_create_character, test_setup_create_character_endpoints,
                test_setup_create_user_with_character,
            },
        };

        /// Expect success when user associated with character is found
        #[tokio::test]
        async fn test_get_or_create_user_found_user() -> Result<(), Error> {
            let test = test_setup_module().await?;
            let character = test_setup_create_character(&test).await?;
            let _ = test_setup_create_user_with_character(&test, character).await?;

            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            // Set character ID in claims to the mock character
            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            let result = user_service.get_or_create_user(claims).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect success when character is found but new user is created
        #[tokio::test]
        async fn test_get_or_create_user_new_user() -> Result<(), Error> {
            let test = test_setup_module().await?;
            let _ = test_setup_create_character(&test).await?;

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
            data::user::user::UserRepository,
            error::Error,
            service::user::{tests::test_setup_module, UserService},
            util::test::setup::{
                test_setup_create_character, test_setup_create_character_endpoints,
                test_setup_create_user_with_character,
            },
        };

        /// Expect no link created when finding character owned by provided user ID
        #[tokio::test]
        async fn test_link_character_owned_success() -> Result<(), Error> {
            let test = test_setup_module().await?;
            let character = test_setup_create_character(&test).await?;
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

        /// Expect link created when an already owned character is transferred to provided user ID
        #[tokio::test]
        async fn test_link_character_owned_transfer_success() -> Result<(), Error> {
            let test = test_setup_module().await?;
            let character = test_setup_create_character(&test).await?;
            let _ = test_setup_create_user_with_character(&test, character).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            let user = user_repo.create().await?;
            let result = user_service.link_character(user.id, claims).await;

            assert!(result.is_ok());
            let link_created = result.unwrap();

            assert!(link_created);

            // TODO: assert character has actually been transferred to the new user ID
            // - Will be done when transfer method is implemented

            Ok(())
        }

        /// Expect link created when character is created but not owned and linked to provided user ID
        #[tokio::test]
        async fn test_link_character_not_owned_success() -> Result<(), Error> {
            let test = test_setup_module().await?;
            let _ = test_setup_create_character(&test).await?;

            let user_repo = UserRepository::new(&test.state.db);
            let user_service = UserService::new(&test.state.db, &test.state.esi_client);

            let mut claims = EveJwtClaims::mock();
            claims.sub = "CHARACTER:EVE:1".to_string();

            let user = user_repo.create().await?;
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

            let user = user_repo.create().await?;
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
            let _ = test_setup_create_character(&test).await?;

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
}
