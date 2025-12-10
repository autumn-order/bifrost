//! Faction service for EVE Online faction data operations.
//!
//! This module provides the `FactionService` for fetching NPC faction information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::faction::FactionRepository,
    error::Error,
    model::db::EveFactionModel,
    service::{
        orchestrator::{
            cache::TrackedTransaction, faction::FactionOrchestrator, OrchestrationCache,
        },
        retry::RetryContext,
    },
};

/// Service for managing EVE Online faction operations.
///
/// Provides methods for fetching NPC faction data from ESI and persisting it to the database.
/// Uses orchestrators to handle dependency resolution and automatic retry logic for transient failures.
pub struct FactionService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> FactionService<'a> {
    /// Creates a new instance of FactionService.
    ///
    /// Constructs a service for managing EVE faction data operations.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    /// - `esi_client` - ESI API client reference
    ///
    /// # Returns
    /// - `FactionService` - New service instance
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Updates all NPC faction information by fetching from ESI and persisting to the database.
    ///
    /// This method handles faction updates with caching support:
    ///
    /// **For no existing factions (empty database):**
    /// - Fetches all faction data from ESI
    /// - Creates all faction records in database
    ///
    /// **For existing factions:**
    /// - Uses HTTP conditional requests (If-Modified-Since) to check for changes
    /// - If ESI returns 304 Not Modified: Only updates the `updated_at` timestamp for all factions
    /// - If ESI returns fresh data: Updates all faction records with new data
    ///
    /// Unlike individual entity services (character/corporation/alliance), this method operates
    /// on all factions at once since ESI only provides a bulk faction endpoint.
    ///
    /// The method uses retry logic to handle transient ESI or database failures automatically.
    /// All database operations are performed within transactions to ensure consistency.
    ///
    /// # Returns
    /// - `Ok(Vec<EveFactionModel>)` - The created or updated faction database records (empty if 304)
    /// - `Err(Error::EsiError)` - Failed to fetch faction data from ESI after retries
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    pub async fn update(&self) -> Result<Vec<EveFactionModel>, Error> {
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry("faction info update", |cache| {
            let db = db.clone();
            let esi_client = esi_client.clone();

            Box::pin(async move {
                let faction_repo = FactionRepository::new(&db);
                let faction_orch = FactionOrchestrator::new(&db, &esi_client);

                // Fetch factions from ESI using the If-Modified-Since within orchestrator
                let Some(fetched_factions) = faction_orch.fetch_factions(cache).await? else {
                    // ESI returned 304 Not Modified, just update all timestamps
                    faction_repo.update_all_timestamps().await?;

                    return Ok(Vec::new());
                };

                // Fresh data received, persist all factions
                let txn = TrackedTransaction::begin(&db).await?;

                let faction_models = faction_orch
                    .persist_factions(&txn, fetched_factions, cache)
                    .await?;

                txn.commit().await?;

                Ok(faction_models)
            })
        })
        .await
    }
}
