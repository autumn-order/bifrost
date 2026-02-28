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

mod alliance;
mod character;
mod corporation;
mod group;
#[macro_use]
mod macros;
pub(crate) mod request;
mod universe;

use std::{sync::Arc, time::Duration};

use alliance::AllianceEndpoints;
use character::CharacterEndpoints;
use corporation::CorporationEndpoints;
use group::EndpointGroup;
use universe::UniverseEndpoints;

/// Size of the sliding window for tracking recent request outcomes.
///
/// The circuit breaker tracks the last N requests (both successes and failures).
/// This provides volume-independent failure detection that works equally well
/// for low-volume and high-volume endpoints.
const ENDPOINT_GROUP_SLIDING_WINDOW_SIZE: usize = 10;

/// Error rate threshold (as a percentage) required to trip the circuit breaker.
///
/// When the error rate within the sliding window exceeds this threshold,
/// the circuit breaker trips and transitions the group to Offline state.
/// For example, with a window size of 20 and threshold of 80%, the circuit
/// trips when 16 or more of the last 20 requests have failed.
const ENDPOINT_GROUP_ERROR_RATE_THRESHOLD: f64 = 0.80;

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
#[derive(Clone)]
pub struct EsiProvider {
    /// ESI API client for making requests
    esi_client: eve_esi::Client,
    /// Collection of endpoint groups with circuit breaker state
    endpoints: Endpoints,
}

/// Container for all ESI endpoint groups.
///
/// Each field represents a logical grouping of related ESI endpoints that share
/// circuit breaker state. This allows fine-grained failure isolation.
#[derive(Clone)]
struct Endpoints {
    /// Alliance-related endpoints (public info, etc.)
    alliance: Arc<EndpointGroup>,
    /// Character-related endpoints (public info, etc.)
    character: Arc<EndpointGroup>,
    /// Corporation-related endpoints (public info, etc.)
    corporation: Arc<EndpointGroup>,
    /// Universe-related endpoints (factions, systems, etc.)
    universe: Arc<EndpointGroup>,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            alliance: Arc::new(EndpointGroup::new("alliance")),
            character: Arc::new(EndpointGroup::new("character")),
            corporation: Arc::new(EndpointGroup::new("corporation")),
            universe: Arc::new(EndpointGroup::new("universe")),
        }
    }
}

impl EsiProvider {
    /// Creates a new ESI provider with circuit breaker protection.
    ///
    /// Initializes all endpoint groups with healthy circuit breaker state.
    ///
    /// # Arguments
    /// - `esi_client` - Reference to configured ESI client
    ///
    /// # Returns
    /// New `EsiProvider` instance ready to serve requests
    pub fn new(esi_client: eve_esi::Client) -> Self {
        Self {
            esi_client,
            endpoints: Endpoints::default(),
        }
    }

    /// Returns a handler for alliance-related ESI endpoints.
    ///
    /// All alliance endpoints share the same circuit breaker state, so repeated
    /// failures on any alliance endpoint will affect the entire group.
    ///
    /// # Returns
    /// `AllianceEndpoints` handler for making alliance-related requests
    pub fn alliance(&self) -> AllianceEndpoints<'_> {
        AllianceEndpoints::new(&self.esi_client, &self.endpoints.alliance)
    }

    /// Returns a handler for character-related ESI endpoints.
    ///
    /// All character endpoints share the same circuit breaker state, so repeated
    /// failures on any character endpoint will affect the entire group.
    ///
    /// # Returns
    /// `CharacterEndpoints` handler for making character-related requests
    pub fn character(&self) -> CharacterEndpoints<'_> {
        CharacterEndpoints::new(&self.esi_client, &self.endpoints.character)
    }

    /// Returns a handler for corporation-related ESI endpoints.
    ///
    /// All corporation endpoints share the same circuit breaker state, so repeated
    /// failures on any corporation endpoint will affect the entire group.
    ///
    /// # Returns
    /// `CorporationEndpoints` handler for making corporation-related requests
    pub fn corporation(&self) -> CorporationEndpoints<'_> {
        CorporationEndpoints::new(&self.esi_client, &self.endpoints.corporation)
    }

    /// Returns a handler for universe-related ESI endpoints.
    ///
    /// All universe endpoints share the same circuit breaker state, so repeated
    /// failures on any universe endpoint will affect the entire group.
    ///
    /// # Returns
    /// `UniverseEndpoints` handler for making universe-related requests
    pub fn universe(&self) -> UniverseEndpoints<'_> {
        UniverseEndpoints::new(&self.esi_client, &self.endpoints.universe)
    }

    /// Returns the underlying ESI client.
    ///
    /// This provides direct access to the `eve_esi::Client` for operations that don't
    /// need circuit breaker protection, such as OAuth2 flows (login, token exchange,
    /// token validation) which should fail fast.
    ///
    /// # Example
    /// ```ignore
    /// // OAuth2 operations
    /// let login = esi_provider.client().oauth2().login_url(scopes)?;
    /// let token = esi_provider.client().oauth2().get_token(code).await?;
    /// let claims = esi_provider.client().oauth2().validate_token(token).await?;
    /// ```
    ///
    /// # Returns
    /// Reference to the underlying `eve_esi::Client`
    pub fn client(&self) -> &eve_esi::Client {
        &self.esi_client
    }
}
