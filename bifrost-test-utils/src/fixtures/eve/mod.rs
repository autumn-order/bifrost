//! EVE Online entity fixture utilities.
//!
//! This module provides methods for creating EVE Online-related test fixtures including
//! factions, alliances, corporations, and characters. It handles both database insertion
//! and mock HTTP endpoint creation for ESI API responses.

use crate::TestContext;

pub mod data;
pub mod factory;
pub mod mock;
pub mod mockito;

impl TestContext {
    /// Access EVE fixture helper methods.
    ///
    /// Returns an EveFixtures instance for creating and managing EVE Online entity
    /// test data during test execution.
    ///
    /// # Arguments
    /// - `self` - Mutable reference to TestContext
    ///
    /// # Returns
    /// - `EveFixtures` - Helper for EVE entity fixture operations
    pub fn eve<'a>(&'a mut self) -> EveFixtures<'a> {
        EveFixtures { setup: self }
    }
}

/// Helper struct for EVE Online entity fixture operations.
///
/// Provides methods for inserting EVE entities (factions, alliances, corporations, characters)
/// into the test database, creating mock data objects, and setting up mock HTTP endpoints.
/// Access via `TestContext::eve()`.
pub struct EveFixtures<'a> {
    pub setup: &'a mut TestContext,
}
