use dioxus_logger::tracing;
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
        change_main: Option<bool>,
    ) -> Result<i32, Error> {
        let user_service = UserService::new(self.db.clone(), self.esi_client.clone());
        let user_character_service =
            UserCharacterService::new(self.db.clone(), self.esi_client.clone());

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

        // If user ID is in session, see if character link or main change is needed
        if let Some(user_id) = user_id {
            let character_id = claims.character_id()?;
            user_character_service
                .link_character(user_id, claims)
                .await?;

            if let Some(true) = change_main {
                user_character_service
                    .change_main(user_id, character_id)
                    .await?;
            }

            tracing::trace!(
                "Returning user ID {} for callback for user currently in session",
                user_id
            );

            return Ok(user_id);
        }

        let user_id = user_service.get_or_create_user(claims).await?;

        tracing::trace!(
            "Returning user ID {} for callback for user not currently in session",
            user_id
        );

        Ok(user_id)
    }
}

#[cfg(test)]
// TODO: Needs unit tests including change main scenario
mod tests {
    use bifrost_test_utils::prelude::*;

    use crate::server::{error::Error, service::auth::callback::CallbackService};

    /// Expect Ok when logging in with a new character
    #[tokio::test]
    async fn creates_new_user() -> Result<(), TestError> {
        let mut test = test_setup_with_user_tables!()?;

        let (corporation_id, mock_corporation) = test.eve().with_mock_corporation(1, None, None);
        let (character_id, mock_character) =
            test.eve()
                .with_mock_character(1, corporation_id, None, None);

        let corporation_endpoint =
            test.eve()
                .with_corporation_endpoint(corporation_id, mock_corporation, 1);
        let character_endpoint =
            test.eve()
                .with_character_endpoint(character_id, mock_character, 1);

        let jwt_endpoints = test.auth().with_jwt_endpoints(character_id, "owner_hash");

        let callback_service = CallbackService::new(&test.state.db, &test.state.esi_client);
        let authorization_code = "test_code";
        let session_user_id = None;
        let result = callback_service
            .handle_callback(&authorization_code, session_user_id, None)
            .await;

        assert!(result.is_ok());

        // Assert JWT keys & token were fetched during callback
        for endpoint in jwt_endpoints {
            endpoint.assert();
        }

        // Assert character & corporation endpoints were fetched during callback when creating character entry
        corporation_endpoint.assert();
        character_endpoint.assert();

        Ok(())
    }

    /// Expect Ok when logging in with an existing character
    #[tokio::test]
    async fn handles_existing_user() -> Result<(), TestError> {
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
            .handle_callback(&authorization_code, Some(user_model.id), None)
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
    async fn fails_when_esi_unavailable() -> Result<(), TestError> {
        let test = test_setup_with_user_tables!()?;

        let callback_service = CallbackService::new(&test.state.db, &test.state.esi_client);
        let authorization_code = "string";
        let result = callback_service
            .handle_callback(authorization_code, None, None)
            .await;

        assert!(matches!(result, Err(Error::EsiError(_))),);

        Ok(())
    }

    /// Expect Error when required database tables are not present
    #[tokio::test]
    async fn fails_when_tables_missing() -> Result<(), TestError> {
        let mut test = test_setup_with_tables!()?;
        let jwt_endpoints = test.auth().with_jwt_endpoints(1, "owner_hash");

        let callback_service = CallbackService::new(&test.state.db, &test.state.esi_client);
        let authorization_code = "string";
        let result = callback_service
            .handle_callback(authorization_code, None, None)
            .await;

        assert!(matches!(result, Err(Error::DbErr(_))));

        // Assert JWT keys & token were fetched during callback
        for endpoint in jwt_endpoints {
            endpoint.assert();
        }

        Ok(())
    }
}
