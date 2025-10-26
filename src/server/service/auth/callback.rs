use oauth2::TokenResponse;
use sea_orm::DatabaseConnection;

use crate::server::{error::Error, service::user::UserService};

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
    pub async fn handle_callback(&self, authorization_code: &str) -> Result<i32, Error> {
        let user_service = UserService::new(&self.db, &self.esi_client);

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

        let character_id = claims.character_id()?;
        let user_id = user_service
            .get_or_create_user(character_id, claims.owner)
            .await?;

        Ok(user_id)
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, DbBackend, DbErr, Schema};

    use crate::server::{
        error::Error,
        service::auth::callback::CallbackService,
        util::test::{
            auth::jwt_mockito::mock_jwt_endpoints,
            eve::mock::{mock_character, mock_corporation},
            mockito::{character::mock_character_endpoint, corporation::mock_corporation_endpoint},
            setup::{test_setup, TestSetup},
        },
    };

    async fn setup() -> Result<TestSetup, DbErr> {
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

    /// Test successful callback
    #[tokio::test]
    async fn test_callback_success() -> Result<(), DbErr> {
        let mut test = setup().await?;
        let (mock_jwt_key_endpoint, mock_jwt_token_endpoint) = mock_jwt_endpoints(&mut test.server);

        let callback_service = CallbackService::new(&test.state.db, &test.state.esi_client);

        // Create the mock character & corporation that will be fetched during callback
        let alliance_id = None;
        let faction_id = None;
        let mock_corporation = mock_corporation(alliance_id, faction_id);

        let corporation_id = 1;
        let mock_character = mock_character(corporation_id, alliance_id, faction_id);

        let expected_requests = 1;
        let corporation_endpoint = mock_corporation_endpoint(
            &mut test.server,
            "/corporations/1",
            mock_corporation,
            expected_requests,
        );
        let character_endpoint = mock_character_endpoint(
            &mut test.server,
            "/characters/1",
            mock_character,
            expected_requests,
        );

        let authorization_code = "test_code";
        let result = callback_service.handle_callback(&authorization_code).await;

        assert!(result.is_ok());

        // Assert JWT keys & token were fetched during callback
        mock_jwt_key_endpoint.assert();
        mock_jwt_token_endpoint.assert();

        // Assert character endpoints were fetched during callback when creating character entry
        character_endpoint.assert();
        corporation_endpoint.assert();

        Ok(())
    }

    /// Test server error when validation fails
    #[tokio::test]
    async fn test_callback_server_error() {
        let test = test_setup().await;
        let callback_service = CallbackService::new(&test.state.db, &test.state.esi_client);

        // Don't create any mock JWT token or key endpoints so that token validation fails

        let code = "string";
        let result = callback_service.handle_callback(code).await;

        assert!(result.is_err());

        assert!(matches!(
            result,
            Err(Error::EsiError(eve_esi::Error::OAuthError(
                eve_esi::OAuthError::RequestTokenError(_)
            )))
        ),)
    }
}
