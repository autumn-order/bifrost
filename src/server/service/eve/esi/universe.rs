//! ESI universe endpoint handlers.
//!
//! This module provides access to EVE Online universe-related ESI endpoints
//! with automatic circuit breaker protection. All endpoints in this module
//! share the same circuit breaker state via the `EndpointGroup`.

use std::sync::Arc;

use eve_esi::model::universe::Faction;

use super::group::EndpointGroup;

/// Handler for ESI universe endpoints.
///
/// Provides access to universe-related ESI endpoints with automatic circuit breaker
/// protection. All methods share a common `EndpointGroup` that tracks the health of
/// universe endpoints collectively.
///
/// # Circuit Breaker Behavior
/// If universe endpoints experience repeated 5xx errors, the circuit breaker will
/// trip and block further requests until a cooldown period expires. This prevents
/// overwhelming ESI when it's experiencing issues.
pub struct UniverseEndpoints<'a> {
    /// ESI client for making API requests
    esi_client: &'a eve_esi::Client,
    /// Shared circuit breaker state for all universe endpoints
    group: &'a Arc<EndpointGroup>,
}

impl<'a> UniverseEndpoints<'a> {
    /// Creates a new universe endpoints handler.
    ///
    /// # Arguments
    /// - `esi_client` - ESI API client reference
    /// - `group` - Shared circuit breaker state for universe endpoints
    ///
    /// # Returns
    /// New `UniverseEndpoints` instance
    pub fn new(esi_client: &'a eve_esi::Client, group: &'a Arc<EndpointGroup>) -> Self {
        Self { esi_client, group }
    }

    define_esi_endpoint! {
        /// Retrieves list of all NPC factions.
        ///
        /// Fetches all NPC faction data including faction IDs, names, descriptions,
        /// solar system information, and corporation/militia details from ESI.
        ///
        /// # Returns
        /// Vector of all factions in EVE Online
        pub fn get_factions(
            &self,
        ) -> EsiProviderRequest<Vec<Faction>>
        =>
        universe, get_factions[]
    }
}
