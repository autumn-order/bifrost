//! Authentication and OAuth2 fixture utilities.
//!
//! This module provides methods for creating authentication-related test fixtures including
//! JWT tokens, JWT keys, and mock OAuth2 endpoints. These fixtures are used to test
//! EVE SSO authentication flows without requiring real EVE Online credentials.

pub mod mock;
pub mod mockito;

use crate::TestContext;

impl TestContext {
    /// Access authentication fixture helper methods.
    ///
    /// Returns an AuthFixtures instance for creating and managing authentication-related
    /// test data during test execution, including JWT tokens and OAuth2 endpoints.
    ///
    /// # Arguments
    /// - `self` - Mutable reference to TestContext
    ///
    /// # Returns
    /// - `AuthFixtures` - Helper for authentication fixture operations
    pub fn auth<'a>(&'a mut self) -> AuthFixtures<'a> {
        AuthFixtures { setup: self }
    }
}

/// Helper struct for authentication fixture operations.
///
/// Provides methods for generating mock JWT tokens, JWT keys, and creating mock
/// OAuth2 HTTP endpoints for testing EVE SSO authentication flows.
/// Access via `TestContext::auth()`.
pub struct AuthFixtures<'a> {
    setup: &'a mut TestContext,
}
