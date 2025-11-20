use eve_esi::model::oauth2::AuthenticationData;

use crate::server::error::Error;

/// Login service to generate URL for login with EVE Online
///
/// Generates a login URL requesting the provided scopes to begin the login process with EVE Online for
/// the user.
///
/// # Arguments
/// - `esi_client` ([`eve_esi::Client`]): ESI Client used to generate the login URL
/// - `scopes` (`Vec<String>`): Scopes to request during login
///
/// # Returns
/// Returns a result containing either:
/// - [`AuthenticationData`]: Login URL to rediret the user to & a CSRF state string for validation in the
///   callback route
/// - [`Error`]: An error if the ESI client is not properly configured for OAuth2
pub fn login_service(
    esi_client: &eve_esi::Client,
    scopes: Vec<String>,
) -> Result<AuthenticationData, Error> {
    let login = esi_client.oauth2().login_url(scopes)?;

    Ok(login)
}

#[cfg(test)]
pub mod tests {
    use bifrost_test_utils::{constant::TEST_USER_AGENT, prelude::*};

    use crate::server::{error::Error, service::auth::login::login_service};

    /// Expect successful generation of login URL
    #[tokio::test]
    async fn generates_login_url() -> Result<(), TestError> {
        let test = test_setup_with_tables!()?;

        let scopes = vec![];
        let result = login_service(&test.state.esi_client, scopes);

        assert!(result.is_ok());

        Ok(())
    }

    /// Expect Error when OAuth2 for ESI client is not configured
    #[test]
    fn fails_when_oauth2_not_configured() -> Result<(), TestError> {
        let esi_client = eve_esi::Client::new(TEST_USER_AGENT).expect("Failed to build ESI client");

        let scopes = vec![];
        let result = login_service(&esi_client, scopes);

        assert!(matches!(
            result,
            Err(Error::EsiError(eve_esi::Error::OAuthError(
                eve_esi::OAuthError::OAuth2NotConfigured
            )))
        ));

        Ok(())
    }
}
