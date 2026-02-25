//! ESI alliance endpoint handlers.
//!
//! This module provides access to EVE Online alliance-related ESI endpoints
//! with automatic circuit breaker protection. All endpoints in this module
//! share the same circuit breaker state via the `EndpointGroup`.

use std::sync::Arc;

use eve_esi::model::alliance::Alliance;

use super::{group::EndpointGroup, macros::define_esi_endpoint};

/// Handler for ESI alliance endpoints.
///
/// Provides access to alliance-related ESI endpoints with automatic circuit breaker
/// protection. All methods share a common `EndpointGroup` that tracks the health of
/// alliance endpoints collectively.
///
/// # Circuit Breaker Behavior
/// If alliance endpoints experience repeated 5xx errors, the circuit breaker will
/// trip and block further requests until a cooldown period expires. This prevents
/// overwhelming ESI when it's experiencing issues.
pub struct AllianceEndpoints<'a> {
    /// ESI client for making API requests
    esi_client: &'a eve_esi::Client,
    /// Shared circuit breaker state for all alliance endpoints
    group: &'a Arc<EndpointGroup>,
}

impl<'a> AllianceEndpoints<'a> {
    /// Creates a new alliance endpoints handler.
    ///
    /// # Arguments
    /// - `esi_client` - ESI API client reference
    /// - `group` - Shared circuit breaker state for alliance endpoints
    ///
    /// # Returns
    /// New `AllianceEndpoints` instance
    pub fn new(esi_client: &'a eve_esi::Client, group: &'a Arc<EndpointGroup>) -> Self {
        Self { esi_client, group }
    }

    define_esi_endpoint! {
        /// Retrieves public information for an alliance.
        ///
        /// Fetches public alliance data including name, ticker, creator corporation,
        /// executor corporation, and founding date from ESI.
        ///
        /// # Arguments
        /// - `alliance_id` - EVE Online alliance ID
        pub fn get_alliance_information(
            &self,
            alliance_id: i64,
        ) -> EsiProviderRequest<Alliance>
        =>
        alliance, get_alliance_information[alliance_id]
    }
}
