//! Database model type aliases.
//!
//! This module provides convenient type aliases for SeaORM database entity models used
//! throughout the application. These aliases simplify type signatures and provide a single
//! point of reference for database model types, making it easier to work with entities
//! without importing from the generated `entity` crate directly.

/// Type alias for Bifrost user database model.
///
/// Represents a user account in the Bifrost system. Each user has a main character and
/// can own multiple EVE Online characters through the `CharacterOwnershipModel` relationship.
///
/// # Fields (from `entity::bifrost_user::Model`)
/// - `id` - Primary key, unique user identifier
/// - `main_character_id` - Foreign key to the user's main character
/// - `created_at` - Timestamp when the user account was created
/// - `updated_at` - Timestamp of the last user record update
pub type UserModel = entity::bifrost_user::Model;

/// Type alias for character ownership database model.
///
/// Represents the relationship between a user and an EVE Online character they own.
/// This model links Bifrost users to their authenticated characters, with each character
/// owned by exactly one user, but users can own multiple characters.
///
/// # Fields (from `entity::bifrost_user_character::Model`)
/// - `id` - Primary key, unique ownership record identifier
/// - `user_id` - Foreign key to the owning user
/// - `character_id` - Foreign key to the owned character
/// - `created_at` - Timestamp when the ownership was established
/// - `updated_at` - Timestamp of the last ownership record update
pub type CharacterOwnershipModel = entity::bifrost_user_character::Model;

/// Type alias for EVE Online character database model.
///
/// Represents cached data for an EVE Online character, including basic information
/// (name, birthday), affiliation (corporation, alliance, faction), and cache timestamps
/// for refresh scheduling.
///
/// # Fields (from `entity::eve_character::Model`)
/// - `id` - Primary key, database identifier
/// - `character_id` - EVE Online character ID (unique)
/// - `name` - Character name
/// - `birthday` - Character creation date in EVE Online
/// - `description` - Character biography/description
/// - `corporation_id` - Current corporation ID
/// - `alliance_id` - Current alliance ID (nullable)
/// - `faction_id` - Current faction ID (nullable)
/// - `info_updated_at` - Timestamp of last character info refresh
/// - `affiliation_updated_at` - Timestamp of last affiliation refresh
/// - `created_at` - Timestamp when record was created in Bifrost
/// - `updated_at` - Timestamp of last record update
pub type EveCharacterModel = entity::eve_character::Model;
