use eve_esi::model::oauth2::AuthenticationData;

use crate::server::error::Error;

pub fn login_service(
    esi_client: &eve_esi::Client,
    scopes: Vec<String>,
) -> Result<AuthenticationData, Error> {
    let login = esi_client.oauth2().login_url(scopes)?;

    Ok(login)
}

#[cfg(test)]
pub mod tests {
    use crate::server::{
        error::Error,
        service::auth::login::login_service,
        util::test::setup::{test_setup, TEST_USER_AGENT},
    };

    /// Test successful login
    #[tokio::test]
    async fn test_login_service() {
        let test = test_setup().await;

        let scopes = vec![];
        let result = login_service(&test.state.esi_client, scopes);

        assert!(result.is_ok())
    }

    /// Test server error when OAuth2 for ESI client is not configured
    #[test]
    fn test_login_server_error() {
        let esi_client = eve_esi::Client::new(TEST_USER_AGENT).expect("Failed to build ESI client");

        let scopes = vec![];
        let result = login_service(&esi_client, scopes);

        assert!(result.is_err());

        assert!(matches!(
            result,
            Err(Error::EsiError(eve_esi::Error::OAuthError(
                eve_esi::OAuthError::OAuth2NotConfigured
            )))
        ))
    }
}
