//! Test configuration constants for EVE ESI client setup.
//!
//! This module defines standard constant values used across all tests for ESI client
//! configuration. These values are not real credentials but placeholder values for
//! testing purposes.

/// User agent string for test ESI client requests.
///
/// Standard user agent format following EVE ESI best practices with contact information
/// and project URL. Used for all mock HTTP requests during testing.
pub static TEST_USER_AGENT: &str =
    "MyApp/1.0 (contact@example.com; +https://github.com/autumn-order/bifrost)";

/// Mock ESI OAuth2 client ID for testing.
///
/// Placeholder client ID used when creating test ESI clients. Not a real credential.
pub static TEST_ESI_CLIENT_ID: &str = "esi_client_id";

/// Mock ESI OAuth2 client secret for testing.
///
/// Placeholder client secret used when creating test ESI clients. Not a real credential.
pub static TEST_ESI_CLIENT_SECRET: &str = "esi_client_secret";

/// Mock OAuth2 callback URL for testing.
///
/// Standard callback URL used in test OAuth2 flows. Points to localhost for testing.
pub static TEST_CALLBACK_URL: &str = "http://localhost:8080/auth/callback";
