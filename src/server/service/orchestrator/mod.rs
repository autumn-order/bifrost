//! Orchestration layer for EVE Online data management.
//!
//! This module provides orchestrators that coordinate complex data operations between
//! ESI API fetching, dependency resolution, and database persistence. Orchestrators handle
//! foreign key dependencies, caching, and ensure data consistency across retry attempts.
//!
//! Each orchestrator manages a specific EVE entity type (factions, alliances, corporations,
//! characters) and provides methods for fetching from ESI, resolving dependencies, and
//! persisting data in the correct order.

pub mod alliance;
pub mod cache;
pub mod character;
pub mod corporation;
pub mod faction;

pub use cache::OrchestrationCache;
