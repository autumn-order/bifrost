use oauth2::TokenResponse;

use crate::server::{error::Error, model::auth::Character};

pub async fn callback_service(
    esi_client: &eve_esi::Client,
    code: &str,
) -> Result<Character, Error> {
    let token = esi_client.oauth2().get_token(code).await?;

    let claims = esi_client
        .oauth2()
        .validate_token(token.access_token().secret().to_string())
        .await?;

    let character_id = claims.character_id()?;

    let character_name = claims.name;
    let character = Character {
        character_id,
        character_name,
    };

    Ok(character)
}

#[cfg(test)]
mod tests {
    use crate::server::{
        error::Error,
        service::auth::callback::callback_service,
        util::test::{auth::jwt_mockito::mock_jwt_endpoints, setup::test_setup},
    };

    /// Test successful callback
    #[tokio::test]
    async fn test_callback_success() {
        let mut test = test_setup().await;
        let (mock_jwt_key_endpoint, mock_jwt_token_endpoint) = mock_jwt_endpoints(&mut test.server);

        let code = "code";
        let result = callback_service(&test.state.esi_client, &code).await;

        // Assert JWT keys & token were fetched during callback
        mock_jwt_key_endpoint.assert();
        mock_jwt_token_endpoint.assert();

        assert!(result.is_ok());
    }

    /// Test server error when validation fails
    #[tokio::test]
    async fn test_callback_server_error() {
        let test = test_setup().await;

        // Don't create any mock JWT token or key endpoints so that token validation fails

        let code = "string";
        let result = callback_service(&test.state.esi_client, code).await;

        assert!(result.is_err());

        assert!(matches!(
            result,
            Err(Error::EsiError(eve_esi::Error::OAuthError(
                eve_esi::OAuthError::RequestTokenError(_)
            )))
        ),)
    }
}
