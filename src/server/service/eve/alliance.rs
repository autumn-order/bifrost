//! Alliance service for EVE Online alliance data operations.
//!
//! This module provides the `AllianceService` for fetching alliance information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use eve_esi::{CacheStrategy, CachedResponse};
use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::server::{
    data::eve::alliance::AllianceRepository, error::Error, model::db::EveAllianceModel,
    service::provider::EveEntityProvider,
};

/// Service for managing EVE Online alliance operations.
///
/// Provides methods for fetching alliance data from ESI and persisting it to the database.
/// Uses orchestrators to handle dependency resolution and automatic retry logic for transient failures.
pub struct AllianceService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> AllianceService<'a> {
    /// Creates a new instance of AllianceService.
    ///
    /// Constructs a service for managing EVE alliance data operations.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    /// - `esi_client` - ESI API client reference
    ///
    /// # Returns
    /// - `AllianceService` - New service instance
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Updates alliance information by fetching from ESI and persisting to the database.
    ///
    /// This method handles alliance updates with caching support:
    ///
    /// **For new alliances (not in database):**
    /// - Fetches complete alliance data from ESI
    /// - Resolves and persists any faction dependencies
    /// - Creates new alliance record in database
    ///
    /// **For existing alliances:**
    /// - Uses HTTP conditional requests (If-Modified-Since) to check for changes
    /// - If ESI returns 304 Not Modified: Only updates the `updated_at` timestamp
    /// - If ESI returns fresh data: Updates all alliance fields and dependencies
    ///
    /// The method uses retry logic to handle transient ESI or database failures automatically.
    /// All database operations are performed within transactions to ensure consistency.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID to fetch and update
    ///
    /// # Returns
    /// - `Ok(EveAllianceModel)` - The created or updated alliance database record
    /// - `Err(Error::EsiError)` - Failed to fetch alliance data from ESI after retries
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    pub async fn update(&self, alliance_id: i64) -> Result<EveAllianceModel, Error> {
        let alliance_repo = AllianceRepository::new(self.db);

        // Build entity provider using one of two strategies:
        // 1. For existing alliances: fetch with conditional request (may return early on 304)
        // 2. For new alliances: fetch unconditionally from ESI
        let eve_entity_provider = match alliance_repo.find_by_eve_id(alliance_id).await? {
            Some(existing_alliance) => {
                // Existing alliance: use if modified since request to check for changes since last update
                let CachedResponse::Fresh(esi_alliance) = self
                    .esi_client
                    .alliance()
                    .get_alliance_information(alliance_id)
                    .send_cached(CacheStrategy::IfModifiedSince(
                        existing_alliance.updated_at.and_utc(),
                    ))
                    .await?
                else {
                    // Alliance data hasn't changed (304), just update the timestamp
                    let refreshed_alliance = alliance_repo
                        .update_info_timestamp(existing_alliance.id)
                        .await?;
                    return Ok(refreshed_alliance);
                };

                // Build provider with pre-fetched data to avoid redundant ESI call
                EveEntityProvider::builder(self.db, self.esi_client)
                    .alliance_with_data(alliance_id, esi_alliance.data)
                    .build()
                    .await?
            }
            None => {
                // New alliance: provider will fetch from ESI during build()
                EveEntityProvider::builder(self.db, self.esi_client)
                    .alliance(alliance_id)
                    .build()
                    .await?
            }
        };

        // Persist alliance and all dependencies (faction) in a transaction
        let txn = self.db.begin().await?;
        let stored_eve_entities = eve_entity_provider.store(&txn).await?;
        txn.commit().await?;

        let alliance = stored_eve_entities.get_alliance_or_err(&alliance_id)?;
        Ok(alliance.clone())
    }
}
