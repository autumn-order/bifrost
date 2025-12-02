//! Corporation service for EVE Online corporation data operations.
//!
//! This module provides the `CorporationService` for fetching corporation information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error,
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

    /// Fetches and persists corporation information from ESI.
    ///
    /// Retrieves complete corporation data from the ESI API and stores it in the database.
    /// If the corporation has alliance or faction affiliations, those dependencies are resolved
    /// and persisted first. Uses retry logic to handle transient ESI or database failures.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID to fetch and store
    ///
    /// # Returns
    /// - `Ok(EveCorporation)` - The created or updated corporation record
    /// - `Err(Error::EsiError)` - Failed to fetch corporation data from ESI
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    pub async fn upsert(
        &self,
        corporation_id: i64,
    ) -> Result<entity::eve_corporation::Model, Error> {
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry(
            &format!("info update for corporation ID {}", corporation_id),
            |cache| {
                let db = db.clone();
                let esi_client = esi_client.clone();

                Box::pin(async move {
                    let corporation_orch = CorporationOrchestrator::new(&db, &esi_client);

                    let fetched_corporation = corporation_orch
                        .fetch_corporation(corporation_id, cache)
                        .await?;

                    let txn = TrackedTransaction::begin(&db).await?;

                    let model = corporation_orch
                        .persist(&txn, corporation_id, fetched_corporation, cache)
                        .await?;

                    txn.commit().await?;

                    Ok(model)
                })
            },
        )
        .await
    }
}
