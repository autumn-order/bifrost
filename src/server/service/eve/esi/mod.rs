//! ESI (EVE Swagger Interface) service provider with circuit breaker protection.
//!
//! This module provides a high-level interface to EVE Online's ESI API with automatic
//! circuit breaker protection for endpoint groups. It organizes ESI endpoints into logical
//! groups (e.g., character, corporation, market) where each group shares circuit breaker
//! state to prevent cascading failures.
//!
//! # Circuit Breaker Pattern
//!
//! Each endpoint group implements a circuit breaker that:
//! - Tracks 5xx errors over a sliding time window
//! - Automatically disables the endpoint group when error threshold is exceeded
//! - Requires a cooldown period before attempting recovery
//! - Gradually recovers with stricter error tolerance
//!
//! # Examples
//!
//! ## Standard Request (Fresh Data)
//!
//! ```ignore
//! let esi_provider = EsiProvider::new(&esi_client);
//!
//! // Make a standard request expecting fresh data (200 OK)
//! let character_info = esi_provider
//!     .character()
//!     .get_character_public_information(123456789)
//!     .send()
//!     .await?;
//! ```
//!
//! ## Cached Request (Conditional)
//!
//! ```ignore
//! use eve_esi::{CacheStrategy, CachedResponse};
//!
//! let esi_provider = EsiProvider::new(&esi_client);
//!
//! // Make a conditional request with If-Modified-Since
//! let response = esi_provider
//!     .character()
//!     .get_character_public_information(123456789)
//!     .send_cached(CacheStrategy::IfModifiedSince(last_updated))
//!     .await?;
//!
//! match response {
//!     CachedResponse::Fresh(esi_response) => {
//!         // ESI returned new data (200 OK)
//!         println!("Character data updated: {:?}", esi_response.data);
//!     }
//!     CachedResponse::NotModified => {
//!         // ESI returned 304 Not Modified - data unchanged
//!         println!("Character data has not changed");
//!     }
//! }
//! ```

mod character;
mod group;
#[macro_use]
mod macros;
pub(crate) mod request;

use std::{sync::Arc, time::Duration};

use character::CharacterEndpoints;
use group::EndpointGroup;

/// Number of 5xx errors within the error window required to trip the circuit breaker.
///
/// When an endpoint group reaches this many errors within `ENDPOINT_GROUP_ERROR_WINDOW`,
/// the circuit breaker trips and transitions the group to Offline state.
const ENDPOINT_GROUP_ERROR_LIMIT: usize = 20;

/// Time window for accumulating errors before resetting the count.
///
/// If no errors occur for this duration while in Impaired state, the endpoint group
/// returns to Healthy. Errors within this window from the first error accumulate
/// towards `ENDPOINT_GROUP_ERROR_LIMIT`.
const ENDPOINT_GROUP_ERROR_WINDOW: Duration = Duration::from_secs(300);

/// Cooldown period after circuit breaker trips before allowing recovery attempts.
///
/// When an endpoint group goes Offline, requests are blocked for this duration.
/// After the cooldown expires, the next request will attempt recovery by transitioning
/// to Recovering state and testing the endpoint.
const ENDPOINT_GROUP_RETRY_COOLDOWN: Duration = Duration::from_secs(60);

/// Main provider for ESI (EVE Swagger Interface) endpoints with circuit breaker protection.
///
/// The `EsiProvider` organizes ESI endpoints into logical groups, each with independent
/// circuit breaker state. This prevents issues with one category of endpoints (e.g., market)
/// from affecting others (e.g., character).
pub struct EsiProvider<'a> {
    /// ESI API client for making requests
    esi_client: &'a eve_esi::Client,
    /// Collection of endpoint groups with circuit breaker state
    endpoints: Endpoints,
}

/// Container for all ESI endpoint groups.
///
/// Each field represents a logical grouping of related ESI endpoints that share
/// circuit breaker state. This allows fine-grained failure isolation.
struct Endpoints {
    /// Character-related endpoints (public info, portraits, etc.)
    character: Arc<EndpointGroup>,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            character: Arc::new(EndpointGroup::default()),
        }
    }
}

impl<'a> EsiProvider<'a> {
    /// Creates a new ESI provider with circuit breaker protection.
    ///
    /// Initializes all endpoint groups with healthy circuit breaker state.
    ///
    /// # Arguments
    /// - `esi_client` - Reference to configured ESI client
    ///
    /// # Returns
    /// New `EsiProvider` instance ready to serve requests
    pub fn new(esi_client: &'a eve_esi::Client) -> Self {
        Self {
            esi_client,
            endpoints: Endpoints::default(),
        }
    }

    /// Returns a handler for character-related ESI endpoints.
    ///
    /// All character endpoints share the same circuit breaker state, so repeated
    /// failures on any character endpoint will affect the entire group.
    ///
    /// # Returns
    /// `CharacterEndpoints` handler for making character-related requests
    pub fn character(&'a self) -> CharacterEndpoints<'a> {
        CharacterEndpoints::new(self.esi_client, &self.endpoints.character)
    }
}
