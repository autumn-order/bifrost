//! Alliance service for EVE Online alliance data operations.
//!
//! This module provides the `AllianceService` for fetching alliance information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use eve_esi::{CacheStrategy, CachedResponse};
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::alliance::AllianceRepository,
    error::Error,
    model::db::EveAllianceModel,
    service::{
        orchestrator::{
            alliance::AllianceOrchestrator, cache::TrackedTransaction, OrchestrationCache,
        },
        retry::RetryContext,
    },
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
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry(
            &format!("info update for alliance ID {}", alliance_id),
            |cache| {
                let db = db.clone();
                let esi_client = esi_client.clone();

                Box::pin(async move {
                    let alliance_repo = AllianceRepository::new(&db);
                    let alliance_orch = AllianceOrchestrator::new(&db, &esi_client);

                    // Check if alliance already exists in database
                    let Some(existing_alliance) = alliance_repo.find_by_eve_id(alliance_id).await?
                    else {
                        // Alliance not in database, fetch information from ESI
                        let esi_data = alliance_orch.fetch_alliance(alliance_id, cache).await?;

                        // Persist fetched alliance to database
                        let txn = TrackedTransaction::begin(&db).await?;

                        let created_alliance = alliance_orch
                            .persist(&txn, alliance_id, esi_data, cache)
                            .await?;

                        txn.commit().await?;

                        return Ok(created_alliance);
                    };

                    // Fetch alliance from ESI, returning 304 not modified if nothing changed since last fetch
                    let CachedResponse::Fresh(fresh_esi_data) = esi_client
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

                        // Return alliance with updated timestamp
                        return Ok(refreshed_alliance);
                    };

                    // Ensure the alliance's dependencies (faction) exist in database
                    alliance_orch
                        .ensure_alliance_dependencies(&[&fresh_esi_data.data], cache)
                        .await?;

                    let txn = TrackedTransaction::begin(&db).await?;

                    let updated_alliance = alliance_orch
                        .persist(&txn, alliance_id, fresh_esi_data.data, cache)
                        .await?;

                    txn.commit().await?;

                    Ok(updated_alliance)
                })
            },
        )
        .await
    }
}
