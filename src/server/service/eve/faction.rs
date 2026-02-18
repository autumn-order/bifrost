//! Faction service for EVE Online faction data operations.
//!
//! This module provides the `FactionService` for fetching NPC faction information from ESI
//! and persisting it to the database.

use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::server::{
    error::Error, model::db::EveFactionModel, service::provider::EveEntityProvider,
};

/// Service for managing EVE Online faction operations.
///
/// Provides methods for fetching NPC faction data from ESI and persisting it to the database.
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
    /// All database operations are performed within transactions to ensure consistency.
    ///
    /// # Returns
    /// - `Ok(Vec<EveFactionModel>)` - The created or updated faction database records (empty if 304)
    /// - `Err(Error::EsiError)` - Failed to fetch faction data from ESI
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn update(&self) -> Result<Vec<EveFactionModel>, Error> {
        // Build entity provider with explicit faction fetch request
        // The builder's fetch_factions_if_stale() handles the conditional request logic
        let eve_entity_provider = EveEntityProvider::builder(self.db, self.esi_client)
            .with_factions()
            .build()
            .await?;

        // Always call store() within a transaction
        // - If fresh data: stores faction data
        // - If 304: updates timestamps only
        // - If not stale: no-op
        let txn = self.db.begin().await?;
        let stored_eve_entities = eve_entity_provider.store(&txn).await?;
        txn.commit().await?;

        // Return stored factions (empty vec if 304 or not stale)
        Ok(stored_eve_entities.get_all_factions())
    }
}
