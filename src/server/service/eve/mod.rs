//! EVE Online service layer.
//!
//! This module contains business logic services for managing EVE Online game data from ESI.
//! Services coordinate data fetching from ESI, orchestrate persistence with dependencies,
//! and handle complex operations like affiliation updates with retry logic and caching.

pub mod affiliation;
pub mod alliance;
pub mod character;
pub mod corporation;
pub mod faction;
