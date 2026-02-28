//! ESI corporation endpoint handlers.
//!
//! This module provides access to EVE Online corporation-related ESI endpoints
//! with automatic circuit breaker protection. All endpoints in this module
//! share the same circuit breaker state via the `EndpointGroup`.

use std::sync::Arc;

use eve_esi::model::corporation::Corporation;

use super::{group::EndpointGroup, macros::define_esi_endpoint};

/// Handler for ESI corporation endpoints.
///
/// Provides access to corporation-related ESI endpoints with automatic circuit breaker
/// protection. All methods share a common `EndpointGroup` that tracks the health of
/// corporation endpoints collectively.
///
/// # Circuit Breaker Behavior
/// If corporation endpoints experience repeated 5xx errors, the circuit breaker will
/// trip and block further requests until a cooldown period expires. This prevents
/// overwhelming ESI when it's experiencing issues.
pub struct CorporationEndpoints<'a> {
    /// ESI client for making API requests
    esi_client: &'a eve_esi::Client,
    /// Shared circuit breaker state for all corporation endpoints
    group: &'a Arc<EndpointGroup>,
}

impl<'a> CorporationEndpoints<'a> {
    /// Creates a new corporation endpoints handler.
    ///
    /// # Arguments
    /// - `esi_client` - ESI API client reference
    /// - `group` - Shared circuit breaker state for corporation endpoints
    ///
    /// # Returns
    /// New `CorporationEndpoints` instance
    pub fn new(esi_client: &'a eve_esi::Client, group: &'a Arc<EndpointGroup>) -> Self {
        Self { esi_client, group }
    }

    define_esi_endpoint! {
        /// Retrieves public information for a corporation.
        ///
        /// Fetches public corporation data including name, ticker, member count, CEO,
        /// alliance membership, and more from ESI.
        ///
        /// # Arguments
        /// - `corporation_id` - EVE Online corporation ID
        pub fn get_corporation_information(
            &self,
            corporation_id: i64,
        ) -> EsiProviderRequest<Corporation>
        =>
        corporation, get_corporation_information[corporation_id]
    }
}
