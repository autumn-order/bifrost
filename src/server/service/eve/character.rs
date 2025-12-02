//! Character service for EVE Online character data operations.
//!
//! This module provides the `CharacterService` for fetching character information from ESI
//! and persisting it to the database. Operations use orchestrators to handle dependencies
//! and include retry logic for reliability.

use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error,
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

    /// Fetches and persists character information from ESI.
    ///
    /// Retrieves complete character data from the ESI API and stores it in the database.
    /// If the character has corporation or faction affiliations, those dependencies are resolved
    /// and persisted first. Uses retry logic to handle transient ESI or database failures.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID to fetch and store
    ///
    /// # Returns
    /// - `Ok(EveCharacter)` - The created or updated character record
    /// - `Err(Error::EsiError)` - Failed to fetch character data from ESI
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    pub async fn upsert(&self, character_id: i64) -> Result<entity::eve_character::Model, Error> {
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry(
            &format!("info update for character ID {}", character_id),
            |cache| {
                let db = db.clone();
                let esi_client = esi_client.clone();

                Box::pin(async move {
                    let character_orch = CharacterOrchestrator::new(&db, &esi_client);

                    let fetched_character =
                        character_orch.fetch_character(character_id, cache).await?;

                    let txn = TrackedTransaction::begin(&db).await?;

                    let model = character_orch
                        .persist(&txn, character_id, fetched_character, cache)
                        .await?;

                    txn.commit().await?;

                    Ok(model)
                })
            },
        )
        .await
    }
}
