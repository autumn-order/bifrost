//! Character service for EVE Online character data operations.
//!
//! This module provides the `CharacterService` for fetching character information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use eve_esi::{CacheStrategy, CachedResponse};
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::character::CharacterRepository,
    error::Error,
    model::db::EveCharacterModel,
    service::{
        orchestrator::{
            cache::TrackedTransaction, character::CharacterOrchestrator, OrchestrationCache,
        },
        retry::RetryContext,
    },
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
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry(
            &format!("info update for character ID {}", character_id),
            |cache| {
                let db = db.clone();
                let esi_client = esi_client.clone();

                Box::pin(async move {
                    let character_repo = CharacterRepository::new(&db);
                    let character_orch = CharacterOrchestrator::new(&db, &esi_client);

                    // Check if character already exists in database
                    let Some(existing_character) =
                        character_repo.find_by_eve_id(character_id).await?
                    else {
                        // Character not in database, fetch information from ESI
                        let esi_data = character_orch.fetch_character(character_id, cache).await?;

                        // Persist fetched character to database
                        let txn = TrackedTransaction::begin(&db).await?;

                        let created_character = character_orch
                            .persist(&txn, character_id, esi_data, cache)
                            .await?;

                        txn.commit().await?;

                        return Ok(created_character);
                    };

                    // Fetch character from ESI, returning 304 not modified if nothing changed since last fetch
                    let CachedResponse::Fresh(fresh_esi_data) = esi_client
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

                        // Return character with updated timestamp
                        return Ok(refreshed_character);
                    };

                    // Ensure the character's dependencies (corporation, faction) exist in database
                    character_orch
                        .ensure_character_dependencies(&[&fresh_esi_data.data], cache)
                        .await?;

                    let txn = TrackedTransaction::begin(&db).await?;

                    let updated_character = character_orch
                        .persist(&txn, character_id, fresh_esi_data.data, cache)
                        .await?;

                    txn.commit().await?;

                    Ok(updated_character)
                })
            },
        )
        .await
    }
}
