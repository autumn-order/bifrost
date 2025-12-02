//! Server configuration management.
//!
//! This module provides the `Config` struct for loading and validating server configuration
//! from environment variables. Configuration includes database URLs, ESI OAuth credentials,
//! contact information, and worker pool sizing. All required environment variables must be
//! present or the application will fail to start with a descriptive error.

use crate::server::error::{config::ConfigError, Error};

/// Server configuration loaded from environment variables.
///
/// Contains all required configuration for running the Bifrost server, including database
/// connection strings, ESI OAuth credentials, contact information for the user agent, and
/// worker pool configuration. The configuration is validated during loading to ensure all
/// required values are present and properly formatted.
///
/// # Environment Variables
/// - `CONTACT_EMAIL` - Contact email for user agent identification (required by ESI)
/// - `ESI_CLIENT_ID` - EVE Online SSO application client ID
/// - `ESI_CLIENT_SECRET` - EVE Online SSO application client secret
/// - `ESI_CALLBACK_URL` - OAuth callback URL registered with EVE Online
/// - `DATABASE_URL` - PostgreSQL database connection string
/// - `VALKEY_URL` - Redis/Valkey connection string for sessions and worker queue
/// - `WORKERS` - Number of worker threads for background job processing (must be a valid number)
pub struct Config {
    /// Contact email address for user agent identification.
    ///
    /// Used in the user agent string sent with ESI requests. Required by ESI to contact
    /// developers in case of API abuse or issues.
    pub contact_email: String,

    /// EVE Online SSO application client ID.
    ///
    /// Obtained from the EVE Online Developer portal when registering an SSO application.
    pub esi_client_id: String,

    /// EVE Online SSO application client secret.
    ///
    /// Secret key for OAuth authentication, obtained from the EVE Developer portal.
    /// Should be kept secure and never committed to version control.
    pub esi_client_secret: String,

    /// OAuth callback URL for EVE Online SSO.
    ///
    /// Must match the callback URL registered in the EVE Developer portal. Users are
    /// redirected here after authenticating with EVE Online.
    pub esi_callback_url: String,

    /// PostgreSQL database connection string.
    ///
    /// Full connection URL including credentials, host, port, and database name.
    /// Example: `postgresql://user:password@localhost:5432/bifrost`
    pub database_url: String,

    /// Redis/Valkey connection string.
    ///
    /// Used for session storage and worker queue backend. Supports standard Redis URL format.
    /// Example: `redis://localhost:6379`
    pub valkey_url: String,

    /// User agent string for ESI requests.
    ///
    /// Automatically generated from package metadata and contact email. Follows ESI's
    /// recommended format: `AppName/Version (Contact; +Repository)`
    pub user_agent: String,

    /// Number of worker threads for background job processing.
    ///
    /// Controls the size of the worker pool that processes background jobs (ESI data refresh,
    /// etc.). Higher values allow more concurrent job processing but consume more resources.
    pub workers: usize,
}

impl Config {
    /// Loads configuration from environment variables.
    ///
    /// Reads and validates all required environment variables, constructing a `Config` instance.
    /// The user agent string is automatically generated from package metadata (name, version,
    /// repository) and the contact email. All required environment variables must be present,
    /// and numeric values must be valid, or this function will return an error.
    ///
    /// # Environment Variables Required
    /// - `CONTACT_EMAIL` - Email for user agent identification
    /// - `ESI_CLIENT_ID` - EVE SSO client ID
    /// - `ESI_CLIENT_SECRET` - EVE SSO client secret
    /// - `ESI_CALLBACK_URL` - OAuth callback URL
    /// - `DATABASE_URL` - PostgreSQL connection string
    /// - `VALKEY_URL` - Redis/Valkey connection string
    /// - `WORKERS` - Number of worker threads (must be parseable as usize)
    ///
    /// # Returns
    /// - `Ok(Config)` - Configuration successfully loaded and validated
    /// - `Err(Error::ConfigError(ConfigError::MissingEnvVar))` - Required environment variable not set
    /// - `Err(Error::ConfigError(ConfigError::InvalidEnvValue))` - Environment variable has invalid format (e.g., WORKERS not a number)
    ///
    /// # Example
    /// ```ignore
    /// // Ensure environment variables are set first
    /// std::env::set_var("CONTACT_EMAIL", "admin@example.com");
    /// std::env::set_var("ESI_CLIENT_ID", "your-client-id");
    /// // ... set other required vars ...
    ///
    /// let config = Config::from_env()?;
    /// println!("User agent: {}", config.user_agent);
    /// // Output: "bifrost/0.1.0 (admin@example.com; +https://github.com/autumn-order/bifrost)"
    /// ```
    pub fn from_env() -> Result<Self, Error> {
        let contact_email = std::env::var("CONTACT_EMAIL")
            .map_err(|_| ConfigError::MissingEnvVar("CONTACT_EMAIL".to_string()))?;
        let user_agent = format!(
            "{}/{} ({}; +{})",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            contact_email,
            env!("CARGO_PKG_REPOSITORY")
        );

        Ok(Self {
            contact_email: contact_email,
            esi_client_id: std::env::var("ESI_CLIENT_ID")
                .map_err(|_| ConfigError::MissingEnvVar("ESI_CLIENT_ID".to_string()))?,
            esi_client_secret: std::env::var("ESI_CLIENT_SECRET")
                .map_err(|_| ConfigError::MissingEnvVar("ESI_CLIENT_SECRET".to_string()))?,
            esi_callback_url: std::env::var("ESI_CALLBACK_URL")
                .map_err(|_| ConfigError::MissingEnvVar("ESI_CALLBACK_URL".to_string()))?,
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| ConfigError::MissingEnvVar("DATABASE_URL".to_string()))?,
            valkey_url: std::env::var("VALKEY_URL")
                .map_err(|_| ConfigError::MissingEnvVar("VALKEY_URL".to_string()))?,
            workers: std::env::var("WORKERS")
                .map_err(|_| ConfigError::MissingEnvVar("WORKERS".to_string()))?
                .parse()
                .map_err(|e| ConfigError::InvalidEnvValue {
                    var: "WORKERS".to_string(),
                    reason: format!("must be a valid number: {}", e),
                })?,
            user_agent,
        })
    }
}
