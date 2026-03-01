//! ESI endpoint handlers organized by category with circuit breaker protection.
//!
//! This module provides organized access to EVE Online's ESI endpoints, grouping them
//! by category (alliance, character, corporation, universe) where each category shares
//! circuit breaker state to prevent cascading failures.
//!
//! # Architecture
//!
//! Each endpoint category (e.g., `alliance`, `character`) is implemented as:
//! - A module containing endpoint handler structs
//! - A handler struct (e.g., `AllianceEndpoints`) that wraps the ESI client
//! - Methods that return `EsiProviderRequest` for deferred execution
//! - Shared `EndpointGroup` for circuit breaker state within the category
//!
//! # Circuit Breaker Grouping
//!
//! Endpoints are grouped by category to provide balanced failure isolation:
//! - **Alliance endpoints** - Alliance public information
//! - **Character endpoints** - Character data, affiliations
//! - **Corporation endpoints** - Corporation public information
//! - **Universe endpoints** - Factions, systems, static universe data
//!
//! If one category experiences repeated failures, only that category is affected.
//! Other categories continue to function normally.
//!
//! # Request Pattern
//!
//! All endpoint methods follow a consistent pattern:
//! 1. Call the endpoint method to create an `EsiProviderRequest`
//! 2. Execute with `.send()` for fresh data or `.send_cached()` for conditional requests
//! 3. Handle the response or error
//!
//! # Examples
//!
//! ## Standard Request
//!
//! ```ignore
//! use bifrost::server::service::eve::esi::EsiProvider;
//!
//! let esi_provider = EsiProvider::new(&esi_client);
//!
//! // Create and execute request for character data
//! let character = esi_provider
//!     .character()
//!     .get_character_public_information(123456789)
//!     .send()
//!     .await?;
//!
//! println!("Character name: {}", character.data.name);
//! ```
//!
//! ## Cached Request with Conditional GET
//!
//! ```ignore
//! use bifrost::server::service::eve::esi::EsiProvider;
//! use eve_esi::{CacheStrategy, CachedResponse};
//!
//! let esi_provider = EsiProvider::new(&esi_client);
//!
//! // Create request with conditional GET
//! let response = esi_provider
//!     .character()
//!     .get_character_public_information(123456789)
//!     .send_cached(CacheStrategy::IfModifiedSince(last_updated))
//!     .await?;
//!
//! match response {
//!     CachedResponse::Fresh(esi_response) => {
//!         println!("Got fresh data: {:?}", esi_response.data);
//!     }
//!     CachedResponse::NotModified => {
//!         println!("Data hasn't changed since last request");
//!     }
//! }
//! ```
//!
//! ## Bulk Operations
//!
//! ```ignore
//! use bifrost::server::service::eve::esi::EsiProvider;
//!
//! let esi_provider = EsiProvider::new(&esi_client);
//!
//! // Fetch affiliations for multiple characters at once
//! let character_ids = vec![123456789, 987654321, 555555555];
//! let affiliations = esi_provider
//!     .character()
//!     .character_affiliation(character_ids)
//!     .send()
//!     .await?;
//!
//! for affiliation in affiliations.data {
//!     println!("{}: corp {}, alliance {:?}",
//!         affiliation.character_id,
//!         affiliation.corporation_id,
//!         affiliation.alliance_id
//!     );
//! }
//! ```
//!
//! ## Handling Circuit Breaker Errors
//!
//! ```ignore
//! use bifrost::server::service::eve::esi::EsiProvider;
//! use bifrost::error::AppError;
//!
//! let esi_provider = EsiProvider::new(&esi_client);
//!
//! match esi_provider
//!     .alliance()
//!     .get_alliance_information(123456)
//!     .send()
//!     .await
//! {
//!     Ok(alliance) => {
//!         println!("Alliance: {}", alliance.data.name);
//!     }
//!     Err(AppError::EsiEndpointOffline { group, .. }) => {
//!         eprintln!("Endpoint group '{}' is offline", group);
//!     }
//!     Err(e) => {
//!         eprintln!("ESI request failed: {}", e);
//!     }
//! }
//! ```
//!
//! # Macros
//!
//! The `macros` module provides `define_esi_endpoint!` which reduces boilerplate
//! when implementing endpoint methods. It automatically:
//! - Wraps ESI client calls in `EsiProviderRequest`
//! - Attaches the endpoint group for circuit breaker logic
//! - Generates consistent documentation for return types and errors
//!
//! See the `macros` module documentation for details on macro usage.

pub mod alliance;
pub mod character;
pub mod corporation;
pub mod universe;
#[macro_use]
mod macros;

use super::group;
