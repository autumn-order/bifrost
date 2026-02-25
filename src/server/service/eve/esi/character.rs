//! ESI character endpoint handlers.
//!
//! This module provides access to EVE Online character-related ESI endpoints
//! with automatic circuit breaker protection. All endpoints in this module
//! share the same circuit breaker state via the `EndpointGroup`.

use std::sync::Arc;

use eve_esi::{model::character::Character, EsiResponse};

use super::group::EndpointGroup;
use crate::server::error::AppError;

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

    /// Retrieves public information for a character.
    ///
    /// Fetches public character data including name, corporation, alliance, birthday,
    /// security status, and more from ESI.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID
    ///
    /// # Returns
    /// - `Ok(EsiResponse<Character>)` - Character data with cache headers
    /// - `Err(AppError::EsiEndpointOffline)` - Circuit breaker is open (endpoint offline)
    /// - `Err(AppError)` - Other errors (network, parsing, etc.)
    pub async fn get_character_public_information(
        &self,
        character_id: i64,
    ) -> Result<EsiResponse<Character>, AppError> {
        // Check status and atomically begin recovery if needed
        let check_result = self.group.check_and_begin_recovery().await?;

        let result = self
            .esi_client
            .character()
            .get_character_public_information(character_id)
            .send()
            .await;

        match &result {
            Err(eve_esi::Error::EsiError(err)) if matches!(err.status, 500..=599) => {
                self.group.handle_5xx_error().await;
            }
            Ok(_) => {
                self.group.maybe_reset_to_healthy(check_result).await;
            }
            _ => {}
        }

        result.map_err(Into::into)
    }
}
