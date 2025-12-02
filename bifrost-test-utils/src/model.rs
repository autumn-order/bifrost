//! Database model type aliases for test utilities.
//!
//! This module provides convenient type aliases for SeaORM database entity models used
//! throughout the test utilities. These aliases match those in the main bifrost crate
//! to ensure consistency across tests.

/// Type alias for Bifrost user database model.
pub type UserModel = entity::bifrost_user::Model;

/// Type alias for character ownership database model.
pub type CharacterOwnershipModel = entity::bifrost_user_character::Model;

/// Type alias for EVE Online character database model.
pub type EveCharacterModel = entity::eve_character::Model;

/// Type alias for EVE Online corporation database model.
pub type EveCorporationModel = entity::eve_corporation::Model;

/// Type alias for EVE Online alliance database model.
pub type EveAllianceModel = entity::eve_alliance::Model;

/// Type alias for EVE Online faction database model.
pub type EveFactionModel = entity::eve_faction::Model;
