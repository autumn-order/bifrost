//! Login service for EVE Online SSO authentication.
//!
//! This module provides the `LoginService` for generating OAuth2 login URLs with EVE SSO.
//! The service initiates the authentication flow by creating URLs with requested scopes
//! and CSRF protection tokens.

use eve_esi::model::oauth2::AuthenticationData;

use crate::server::{error::AppError, service::eve::esi::EsiProvider};

/// Service for generating EVE Online SSO login URLs.
///
/// Provides methods for initiating the OAuth2 authentication flow by generating
/// login URLs with requested scopes and CSRF state tokens for validation.
pub struct LoginService<'a> {
    esi_provider: &'a EsiProvider,
}

impl<'a> LoginService<'a> {
    /// Creates a new instance of LoginService.
    ///
    /// Constructs a service for generating EVE SSO login URLs.
    ///
    /// # Arguments
    /// - `esi_provider` - ESI provider reference (OAuth2 accessed via `.client().oauth2()`)
    ///
    /// # Returns
    /// - `LoginService` - New service instance
    pub fn new(esi_provider: &'a EsiProvider) -> Self {
        Self { esi_provider }
    }

    /// Generates an OAuth2 login URL for EVE Online SSO.
    ///
    /// Creates a login URL with the requested scopes and a CSRF state token for security.
    /// The user should be redirected to this URL to begin the authentication flow with EVE Online.
    ///
    /// # Arguments
    /// - `scopes` - List of OAuth2 scopes to request from the user
    ///
    /// # Returns
    /// - `Ok(AuthenticationData)` - Login URL and CSRF state token for callback validation
    /// - `Err(AppError::Esi)` - ESI client OAuth2 not configured properly
    pub fn generate_login_url(&self, scopes: Vec<String>) -> Result<AuthenticationData, AppError> {
        let login = self.esi_provider.client().oauth2().login_url(scopes)?;

        Ok(login)
    }
}

#[cfg(test)]
/// Tests for login service functionality.
pub mod tests {
    use bifrost_test_utils::{constant::TEST_USER_AGENT, prelude::*};

    use crate::server::{
        error::AppError,
        service::{auth::login::LoginService, eve::esi::EsiProvider},
    };

    /// Expect successful generation of login URL
    #[tokio::test]
    async fn generates_login_url() -> Result<(), TestError> {
        let test = TestBuilder::new().build().await?;

        let esi_provider = EsiProvider::new(test.esi_client);
        let login_service = LoginService::new(&esi_provider);
        let scopes = vec![];
        let result = login_service.generate_login_url(scopes);

        assert!(result.is_ok());

        Ok(())
    }

    /// Expect Error when OAuth2 for ESI client is not configured
    #[test]
    fn fails_when_oauth2_not_configured() -> Result<(), TestError> {
        let esi_client = eve_esi::Client::new(TEST_USER_AGENT).expect("Failed to build ESI client");

        let esi_provider = EsiProvider::new(esi_client);
        let login_service = LoginService::new(&esi_provider);
        let scopes = vec![];
        let result = login_service.generate_login_url(scopes);

        assert!(matches!(
            result,
            Err(AppError::Esi(eve_esi::Error::OAuthError(
                eve_esi::OAuthError::OAuth2NotConfigured
            )))
        ));

        Ok(())
    }
}
