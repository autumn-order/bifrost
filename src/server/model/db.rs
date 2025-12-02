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

/// Type alias for EVE Online corporation database model.
///
/// Represents cached data for an EVE Online corporation, including basic information
/// (name, ticker), leadership (CEO, creator), affiliation (alliance, faction), and
/// various corporation details like member count, tax rate, and founding date.
///
/// # Fields (from `entity::eve_corporation::Model`)
/// - `id` - Primary key, database identifier
/// - `corporation_id` - EVE Online corporation ID (unique)
/// - `alliance_id` - Current alliance ID (nullable, foreign key)
/// - `faction_id` - Current faction ID (nullable, foreign key)
/// - `ceo_id` - Character ID of the corporation CEO
/// - `creator_id` - Character ID of the corporation creator
/// - `date_founded` - Date the corporation was founded (nullable)
/// - `description` - Corporation description/bio (nullable)
/// - `home_station_id` - Home station ID (nullable)
/// - `member_count` - Number of members in the corporation
/// - `name` - Corporation name
/// - `shares` - Number of shares (nullable)
/// - `tax_rate` - Corporation tax rate (0.0 to 100.0)
/// - `ticker` - Corporation ticker symbol
/// - `url` - Corporation website URL (nullable)
/// - `war_eligible` - Whether the corporation is eligible for wars (nullable)
/// - `created_at` - Timestamp when record was created in Bifrost
/// - `info_updated_at` - Timestamp of last corporation info refresh
/// - `affiliation_updated_at` - Timestamp of last affiliation refresh
pub type EveCorporationModel = entity::eve_corporation::Model;

/// Type alias for EVE Online alliance database model.
///
/// Represents cached data for an EVE Online alliance, including basic information
/// (name, ticker), leadership (creator, executor corporation), and founding information.
///
/// # Fields (from `entity::eve_alliance::Model`)
/// - `id` - Primary key, database identifier
/// - `alliance_id` - EVE Online alliance ID (unique)
/// - `faction_id` - Current faction ID (nullable, foreign key)
/// - `creator_corporation_id` - Corporation ID that created the alliance
/// - `executor_corporation_id` - Current executor corporation ID (nullable)
/// - `creator_id` - Character ID of the alliance creator
/// - `date_founded` - Date the alliance was founded
/// - `name` - Alliance name
/// - `ticker` - Alliance ticker symbol
/// - `created_at` - Timestamp when record was created in Bifrost
/// - `updated_at` - Timestamp of last record update
pub type EveAllianceModel = entity::eve_alliance::Model;

/// Type alias for EVE Online faction database model.
///
/// Represents cached data for an EVE Online NPC faction, including basic information
/// (name, description), associated corporations, militia information, and territorial data.
///
/// # Fields (from `entity::eve_faction::Model`)
/// - `id` - Primary key, database identifier
/// - `faction_id` - EVE Online faction ID (unique)
/// - `corporation_id` - Associated corporation ID (nullable)
/// - `militia_corporation_id` - Militia corporation ID (nullable)
/// - `description` - Faction description
/// - `is_unique` - Whether this is a unique faction
/// - `name` - Faction name
/// - `size_factor` - Size factor of the faction
/// - `solar_system_id` - Home solar system ID (nullable)
/// - `station_count` - Number of stations owned by the faction
/// - `station_system_count` - Number of systems with stations
/// - `created_at` - Timestamp when record was created in Bifrost
/// - `updated_at` - Timestamp of last record update
pub type EveFactionModel = entity::eve_faction::Model;
