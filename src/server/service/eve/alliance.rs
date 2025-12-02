//! Alliance service for EVE Online alliance data operations.
//!
//! This module provides the `AllianceService` for fetching alliance information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use sea_orm::DatabaseConnection;

use crate::server::{
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

    /// Fetches and persists alliance information from ESI.
    ///
    /// Retrieves complete alliance data from the ESI API and stores it in the database.
    /// If the alliance has faction affiliations, those dependencies are resolved and persisted
    /// first. Uses retry logic to handle transient ESI or database failures.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID to fetch and store
    ///
    /// # Returns
    /// - `Ok(EveAlliance)` - The created or updated alliance record
    /// - `Err(Error::EsiError)` - Failed to fetch alliance data from ESI
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    pub async fn upsert(&self, alliance_id: i64) -> Result<EveAllianceModel, Error> {
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry(
            &format!("info update for alliance ID {}", alliance_id),
            |cache| {
                let db = db.clone();
                let esi_client = esi_client.clone();

                Box::pin(async move {
                    let alliance_orch = AllianceOrchestrator::new(&db, &esi_client);

                    let fetched_alliance = alliance_orch.fetch_alliance(alliance_id, cache).await?;

                    let txn = TrackedTransaction::begin(&db).await?;

                    let model = alliance_orch
                        .persist(&txn, alliance_id, fetched_alliance, cache)
                        .await?;

                    txn.commit().await?;

                    Ok(model)
                })
            },
        )
        .await
    }
}
