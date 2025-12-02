//! EVE Online entity-specific scheduling implementations.
//!
//! This module contains schedulers for different EVE Online entity types, each implementing
//! the `SchedulableEntity` trait to enable automatic cache refresh scheduling. Each submodule
//! handles a specific entity type (factions, alliances, corporations, characters, and
//! character affiliations) with appropriate batch sizing and scheduling logic.

pub mod affiliation;
pub mod alliance;
pub mod character;
pub mod corporation;
pub mod faction;
