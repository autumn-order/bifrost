//! EVE Online data repositories.
//!
//! This module contains repositories for managing EVE Online game data from the ESI API.
//! Each repository handles a specific entity type (characters, corporations, alliances, factions)
//! and provides methods for upserting data from ESI and querying database records.

pub mod alliance;
pub mod character;
pub mod corporation;
pub mod faction;

#[cfg(test)]
mod tests;
