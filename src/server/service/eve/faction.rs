//! Faction service for EVE Online faction data operations.
//!
//! This module provides the `FactionService` for fetching NPC faction information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use sea_orm::DatabaseConnection;

use crate::server::{
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

    /// Fetches and persists all NPC faction information from ESI.
    ///
    /// Retrieves the complete list of NPC factions from the ESI API and stores them in the database.
    /// Only fetches new data if the cache period has expired (cache expires at 11:05 UTC after downtime).
    /// Uses retry logic to handle transient ESI or database failures.
    ///
    /// # Returns
    /// - `Ok(Vec<EveFaction>)` - List of created or updated faction records (empty if cache valid)
    /// - `Err(Error::EsiError)` - Failed to fetch faction data from ESI
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    pub async fn update_factions(&self) -> Result<Vec<EveFactionModel>, Error> {
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry("faction info update", |cache| {
            let db = db.clone();
            let esi_client = esi_client.clone();

            Box::pin(async move {
                let faction_orch = FactionOrchestrator::new(&db, &esi_client);

                let Some(fetched_factions) = faction_orch.fetch_factions(cache).await? else {
                    return Ok(Vec::new());
                };

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
