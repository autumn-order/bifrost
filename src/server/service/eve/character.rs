//! Character service for EVE Online character data operations.
//!
//! This module provides the `CharacterService` for fetching character information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use eve_esi::{CacheStrategy, CachedResponse};
use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::server::{
    data::eve::character::CharacterRepository, error::Error, model::db::EveCharacterModel,
    service::provider::EveEntityProvider,
};

/// Service for managing EVE Online character operations.
///
/// Provides methods for fetching character data from ESI and persisting it to the database.
/// Uses orchestrators to handle dependency resolution and automatic retry logic for transient failures.
pub struct CharacterService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CharacterService<'a> {
    /// Creates a new instance of CharacterService.
    ///
    /// Constructs a service for managing EVE character data operations.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    /// - `esi_client` - ESI API client reference
    ///
    /// # Returns
    /// - `CharacterService` - New service instance
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Updates character information by fetching from ESI and persisting to the database.
    ///
    /// This method handles character updates with caching support:
    ///
    /// **For new characters (not in database):**
    /// - Fetches complete character data from ESI
    /// - Resolves and persists any corporation/faction dependencies
    /// - Creates new character record in database
    ///
    /// **For existing characters:**
    /// - Uses HTTP conditional requests (If-Modified-Since) to check for changes
    /// - If ESI returns 304 Not Modified: Only updates the `info_updated_at` timestamp
    /// - If ESI returns fresh data: Updates all character fields and dependencies
    ///
    /// The method uses retry logic to handle transient ESI or database failures automatically.
    /// All database operations are performed within transactions to ensure consistency.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID to fetch and update
    ///
    /// # Returns
    /// - `Ok(EveCharacterModel)` - The created or updated character database record
    /// - `Err(Error::EsiError)` - Failed to fetch character data from ESI after retries
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    pub async fn update(&self, character_id: i64) -> Result<EveCharacterModel, Error> {
        let character_repo = CharacterRepository::new(self.db);

        // Build entity provider using one of two strategies:
        // 1. For existing characters: fetch with conditional request (may return early on 304)
        // 2. For new characters: fetch unconditionally from ESI
        let eve_entity_provider = match character_repo.find_by_eve_id(character_id).await? {
            Some(existing_character) => {
                // Existing character: use if modified since request to check for changes since last update
                let CachedResponse::Fresh(esi_character) = self
                    .esi_client
                    .character()
                    .get_character_public_information(character_id)
                    .send_cached(CacheStrategy::IfModifiedSince(
                        existing_character.info_updated_at.and_utc(),
                    ))
                    .await?
                else {
                    // Character data hasn't changed (304), just update the timestamp
                    let refreshed_character = character_repo
                        .update_info_timestamp(existing_character.id)
                        .await?;
                    return Ok(refreshed_character);
                };

                // Build provider with pre-fetched data to avoid redundant ESI call
                EveEntityProvider::builder(self.db, self.esi_client)
                    .character_with_data(character_id, esi_character.data)
                    .build()
                    .await?
            }
            None => {
                // New character: provider will fetch from ESI during build()
                EveEntityProvider::builder(self.db, self.esi_client)
                    .character(character_id)
                    .build()
                    .await?
            }
        };

        // Persist character and all dependencies (corporation, alliance, faction) in a transaction
        let txn = self.db.begin().await?;
        let stored_eve_entities = eve_entity_provider.store(&txn).await?;
        txn.commit().await?;

        let character = stored_eve_entities.get_character_or_err(&character_id)?;
        Ok(character.clone())
    }
}
