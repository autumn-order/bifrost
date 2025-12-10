//! Corporation service for EVE Online corporation data operations.
//!
//! This module provides the `CorporationService` for fetching corporation information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use eve_esi::{CacheStrategy, CachedResponse};
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::corporation::CorporationRepository,
    error::Error,
    model::db::EveCorporationModel,
    service::{
        orchestrator::{
            cache::TrackedTransaction, corporation::CorporationOrchestrator, OrchestrationCache,
        },
        retry::RetryContext,
    },
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
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry(
            &format!("info update for corporation ID {}", corporation_id),
            |cache| {
                let db = db.clone();
                let esi_client = esi_client.clone();

                Box::pin(async move {
                    let corporation_repo = CorporationRepository::new(&db);
                    let corporation_orch = CorporationOrchestrator::new(&db, &esi_client);

                    // Check if corporation already exists in database
                    let Some(existing_corp) =
                        corporation_repo.find_by_eve_id(corporation_id).await?
                    else {
                        // Corporation not in database, fetch information from ESI
                        let esi_data = corporation_orch
                            .fetch_corporation(corporation_id, cache)
                            .await?;

                        // Persist fetched corporation to database
                        let txn = TrackedTransaction::begin(&db).await?;

                        let created_corp = corporation_orch
                            .persist(&txn, corporation_id, esi_data, cache)
                            .await?;

                        txn.commit().await?;

                        return Ok(created_corp);
                    };

                    // Fetch corporation from ESI, returning 304 not modified if nothing changed since last fetch
                    let CachedResponse::Fresh(fresh_corporation_info) = esi_client
                        .corporation()
                        .get_corporation_information(corporation_id)
                        .send_cached(CacheStrategy::IfModifiedSince(
                            existing_corp.info_updated_at.and_utc(),
                        ))
                        .await?
                    else {
                        // Corporation data hasn't changed (304), just update the timestamp
                        let refreshed_corp = corporation_repo
                            .update_info_timestamp(existing_corp.id)
                            .await?;

                        // Return corporation with updated timestamp
                        return Ok(refreshed_corp);
                    };

                    // Ensure the corporation's dependencies (alliance, faction) exist in database
                    corporation_orch
                        .ensure_corporation_dependencies(&[&fresh_corporation_info.data], cache)
                        .await?;

                    let txn = TrackedTransaction::begin(&db).await?;

                    let updated_corp = corporation_orch
                        .persist(&txn, corporation_id, fresh_corporation_info.data, cache)
                        .await?;

                    txn.commit().await?;

                    Ok(updated_corp)
                })
            },
        )
        .await
    }
}
