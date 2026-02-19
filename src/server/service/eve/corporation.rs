//! Corporation service for EVE Online corporation data operations.
//!
//! This module provides the `CorporationService` for fetching corporation information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use eve_esi::{CacheStrategy, CachedResponse};
use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::server::{
    data::eve::corporation::CorporationRepository, error::Error, model::db::EveCorporationModel,
    service::eve::orchestrator::EveEntityOrchestrator,
};

/// Service for managing EVE Online corporation operations.
///
/// Provides methods for fetching corporation data from ESI and persisting it to the database.
/// Uses orchestrators to handle dependency resolution and automatic retry logic for transient failures.
pub struct CorporationService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CorporationService<'a> {
    /// Creates a new instance of CorporationService.
    ///
    /// Constructs a service for managing EVE corporation data operations.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    /// - `esi_client` - ESI API client reference
    ///
    /// # Returns
    /// - `CorporationService` - New service instance
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Updates corporation information by fetching from ESI and persisting to the database.
    ///
    /// This method handles corporation updates with caching support:
    ///
    /// **For new corporations (not in database):**
    /// - Fetches complete corporation data from ESI
    /// - Resolves and persists any alliance/faction dependencies
    /// - Creates new corporation record in database
    ///
    /// **For existing corporations:**
    /// - Uses HTTP conditional requests (If-Modified-Since) to check for changes
    /// - If ESI returns 304 Not Modified: Only updates the `info_updated_at` timestamp
    /// - If ESI returns fresh data: Updates all corporation fields and dependencies
    ///
    /// The method uses retry logic to handle transient ESI or database failures automatically.
    /// All database operations are performed within transactions to ensure consistency.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID to fetch and update
    ///
    /// # Returns
    /// - `Ok(EveCorporationModel)` - The created or updated corporation database record
    /// - `Err(Error::EsiError)` - Failed to fetch corporation data from ESI after retries
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    pub async fn update(&self, corporation_id: i64) -> Result<EveCorporationModel, Error> {
        let corporation_repo = CorporationRepository::new(self.db);

        // Build entity provider using one of two strategies:
        // 1. For existing corporations: fetch with conditional request (may return early on 304)
        // 2. For new corporations: fetch unconditionally from ESI
        let eve_entity_orchestrator = match corporation_repo.find_by_eve_id(corporation_id).await? {
            Some(existing_corporation) => {
                // Existing corporation: use if modified since request to check for changes since last update
                let CachedResponse::Fresh(esi_corporation) = self
                    .esi_client
                    .corporation()
                    .get_corporation_information(corporation_id)
                    .send_cached(CacheStrategy::IfModifiedSince(
                        existing_corporation.info_updated_at.and_utc(),
                    ))
                    .await?
                else {
                    // Corporation data hasn't changed (304), just update the timestamp
                    let refreshed_corporation = corporation_repo
                        .update_info_timestamp(existing_corporation.id)
                        .await?;
                    return Ok(refreshed_corporation);
                };

                // Build orchestrator with pre-fetched data to avoid redundant ESI call
                EveEntityOrchestrator::builder(self.db, self.esi_client)
                    .corporation_with_data(corporation_id, esi_corporation.data)
                    .build()
                    .await?
            }
            None => {
                // New corporation: orchestrator will fetch from ESI during build()
                EveEntityOrchestrator::builder(self.db, self.esi_client)
                    .corporation(corporation_id)
                    .build()
                    .await?
            }
        };

        // Persist corporation and all dependencies (alliance, faction) in a transaction
        let txn = self.db.begin().await?;
        let stored_eve_entities = eve_entity_orchestrator.store(&txn).await?;
        txn.commit().await?;

        let corporation = stored_eve_entities.get_corporation_or_err(&corporation_id)?;
        Ok(corporation.clone())
    }
}
