use oauth2::TokenResponse;
use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error,
    service::user::{user_character::UserCharacterService, UserService},
};

pub struct CallbackService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CallbackService<'a> {
    /// Creates a new instance of [`CallbackService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Callback service which fetches & validates JWT token after successful login
    ///
    /// Uses an authorization code to fetch a JWT token which provides the user's character name,
    /// ID, as well as an access & refresh token used for fetching data related to the requested scopes.
    ///
    /// - A character will then be either found in the database or created after fetching character information
    ///   from ESI using the character's ID found in the token.
    /// - The database will then be checked to see if any user owns the character, if not a new user will be
    ///   created and linked to the character.
    ///
    /// # Arguments
    /// - `db`: (&[`DatabaseConnection`]): Connection to access database
    /// - `esi_client` ([`eve_esi::Client`]): ESI Client used to fetch and validate JWT token
    /// - `code` (`&str`): Authorization code used to fetch JWT token
    ///
    /// # Returns
    /// Returns a result containing either:
    /// - `i32`: ID of the user after sucessful callback
    /// - [`Error`]: An error if JWT token fetching or validation fails
    pub async fn handle_callback(
        &self,
        authorization_code: &str,
        user_id: Option<i32>,
    ) -> Result<i32, Error> {
        let user_service = UserService::new(&self.db, &self.esi_client);
        let user_character_service = UserCharacterService::new(&self.db, &self.esi_client);

        let token = self
            .esi_client
            .oauth2()
            .get_token(authorization_code)
            .await?;
        let claims = self
            .esi_client
            .oauth2()
            .validate_token(token.access_token().secret().to_string())
            .await?;

        if let Some(user_id) = user_id {
            user_character_service
                .link_character(user_id, claims)
                .await?;

            return Ok(user_id);
        }

        let user_id = user_service.get_or_create_user(claims).await?;

        Ok(user_id)
    }
}

#[cfg(test)]
mod tests {
    use bifrost_test_utils::prelude::*;

    use crate::server::{error::Error, service::auth::callback::CallbackService};

    /// Expect Ok when logging in with a new character
    #[tokio::test]
    async fn test_callback_new_user_success() -> Result<(), TestError> {
        let mut test = test_setup_with_user_tables!()?;
        let character_id = 1;
        let character_endpoints =
            test.eve()
                .with_character_endpoint(character_id, 1, None, None, 1);
        let jwt_endpoints = test.auth().with_jwt_endpoints(character_id, "owner_hash");

        let callback_service = CallbackService::new(&test.state.db, &test.state.esi_client);
        let authorization_code = "test_code";
        let session_user_id = None;
        let result = callback_service
            .handle_callback(&authorization_code, session_user_id)
            .await;

        assert!(result.is_ok());

        // Assert JWT keys & token were fetched during callback
        for endpoint in jwt_endpoints {
            endpoint.assert();
        }

        // Assert character endpoints were fetched during callback when creating character entry
        for endpoint in character_endpoints {
            endpoint.assert();
        }

        Ok(())
    }

    /// Expect Ok when logging in with an existing character
    #[tokio::test]
    async fn test_callback_existing_user_success() -> Result<(), TestError> {
        let mut test = test_setup_with_user_tables!()?;
        let (user_model, user_character_model, character_model) = test
            .user()
            .insert_user_with_mock_character(1, 1, None, None)
            .await?;
        let jwt_endpoints = test.auth().with_jwt_endpoints(
            character_model.character_id,
            &user_character_model.owner_hash,
        );

        let callback_service = CallbackService::new(&test.state.db, &test.state.esi_client);

        let authorization_code = "test_code";
        let result = callback_service
            .handle_callback(&authorization_code, Some(user_model.id))
            .await;

        assert!(result.is_ok(), "Error: {:#?}", result);

        // Assert JWT keys & token were fetched during callback
        for endpoint in jwt_endpoints {
            endpoint.assert();
        }

        Ok(())
    }

    /// Expect Error when ESI endpoints are unavailable
    #[tokio::test]
    async fn test_callback_server_error() -> Result<(), TestError> {
        let test = test_setup_with_user_tables!()?;

        let callback_service = CallbackService::new(&test.state.db, &test.state.esi_client);
        let authorization_code = "string";
        let result = callback_service
            .handle_callback(authorization_code, None)
            .await;

        assert!(matches!(result, Err(Error::EsiError(_))),);

        Ok(())
    }

    /// Expect Error required database tables are not presents
    #[tokio::test]
    async fn handle_callback_err_missing_tables() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!()?;
        let jwt_endpoints = test.auth().with_jwt_endpoints(1, "owner_hash");

        let callback_service = CallbackService::new(&test.state.db, &test.state.esi_client);
        let authorization_code = "string";
        let result = callback_service
            .handle_callback(authorization_code, None)
            .await;

        assert!(matches!(result, Err(Error::DbErr(_))));

        // Assert JWT keys & token were fetched during callback
        for endpoint in jwt_endpoints {
            endpoint.assert();
        }

        Ok(())
    }
}
