//! ESI character endpoint handlers.
//!
//! This module provides access to EVE Online character-related ESI endpoints
//! with automatic circuit breaker protection. All endpoints in this module
//! share the same circuit breaker state via the `EndpointGroup`.

use std::sync::Arc;

use eve_esi::model::character::Character;

use super::{group::EndpointGroup, macros::define_esi_endpoint};

/// Handler for ESI character endpoints.
///
/// Provides access to character-related ESI endpoints with automatic circuit breaker
/// protection. All methods share a common `EndpointGroup` that tracks the health of
/// character endpoints collectively.
///
/// # Circuit Breaker Behavior
/// If character endpoints experience repeated 5xx errors, the circuit breaker will
/// trip and block further requests until a cooldown period expires. This prevents
/// overwhelming ESI when it's experiencing issues.
pub struct CharacterEndpoints<'a> {
    /// ESI client for making API requests
    esi_client: &'a eve_esi::Client,
    /// Shared circuit breaker state for all character endpoints
    group: &'a Arc<EndpointGroup>,
}

impl<'a> CharacterEndpoints<'a> {
    /// Creates a new character endpoints handler.
    ///
    /// # Arguments
    /// - `esi_client` - ESI API client reference
    /// - `group` - Shared circuit breaker state for character endpoints
    ///
    /// # Returns
    /// New `CharacterEndpoints` instance
    pub fn new(esi_client: &'a eve_esi::Client, group: &'a Arc<EndpointGroup>) -> Self {
        Self { esi_client, group }
    }

    define_esi_endpoint! {
        /// Retrieves public information for a character.
        ///
        /// Fetches public character data including name, corporation, alliance, birthday,
        /// security status, and more from ESI.
        ///
        /// # Arguments
        /// - `character_id` - EVE Online character ID
        pub fn get_character_public_information(
            &self,
            character_id: i64,
        ) -> EsiProviderRequest<Character>
        =>
        character, get_character_public_information[character_id]
    }
}
