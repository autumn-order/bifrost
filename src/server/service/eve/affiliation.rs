//! Affiliation service for EVE Online character and corporation affiliation updates.
//!
//! This module provides the `AffiliationService` for bulk updating character and corporation
//! affiliations from ESI. It handles fetching affiliation data, resolving dependencies,
//! and updating relationships in a single transaction with retry logic.

use std::collections::HashSet;

use dioxus_logger::tracing;
use eve_esi::model::character::CharacterAffiliation;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::{character::CharacterRepository, corporation::CorporationRepository},
    error::Error,
    service::{
        orchestrator::{
            alliance::AllianceOrchestrator, cache::TrackedTransaction,
            character::CharacterOrchestrator, corporation::CorporationOrchestrator,
            faction::FactionOrchestrator, OrchestrationCache,
        },
        retry::RetryContext,
    },
    util::eve::{is_valid_character_id, ESI_AFFILIATION_REQUEST_LIMIT},
};

/// Service for managing EVE Online affiliation updates.
///
/// Provides methods for bulk updating character and corporation affiliations from ESI.
/// Handles dependency resolution for factions, alliances, and corporations, and updates
/// all relationships in a single transaction with automatic retry logic.
pub struct AffiliationService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> AffiliationService<'a> {
    /// Creates a new instance of AffiliationService.
    ///
    /// Constructs a service for managing EVE affiliation data operations.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    /// - `esi_client` - ESI API client reference
    ///
    /// # Returns
    /// - `AffiliationService` - New service instance
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Updates character and corporation affiliations from ESI in bulk.
    ///
    /// Fetches character affiliation data from ESI and updates both character-to-corporation
    /// and corporation-to-alliance relationships. Automatically resolves and fetches any missing
    /// dependencies (factions, alliances, corporations) to satisfy foreign key constraints.
    /// All updates are performed in a single transaction. Uses retry logic for transient failures.
    ///
    /// The method validates and sanitizes input character IDs, caps the request to ESI limits,
    /// and skips invalid IDs to prevent request failures.
    ///
    /// # Arguments
    /// - `character_ids` - List of EVE character IDs to update affiliations for (max 1000)
    ///
    /// # Returns
    /// - `Ok(())` - All affiliations successfully updated
    /// - `Err(Error::EsiError)` - Failed to fetch affiliation or dependency data from ESI
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    pub async fn update_affiliations(&self, character_ids: Vec<i64>) -> Result<(), Error> {
        // Cap character_ids to ESI limit to prevent affiliation request from erroring due to exceeding limit
        let character_ids = if character_ids.len() > ESI_AFFILIATION_REQUEST_LIMIT {
            tracing::warn!(
                "Received {} character IDs for affiliation update, exceeding ESI limit of {}; truncating to limit",
                character_ids.len(),
                ESI_AFFILIATION_REQUEST_LIMIT
            );
            character_ids
                .into_iter()
                .take(ESI_AFFILIATION_REQUEST_LIMIT)
                .collect()
        } else {
            character_ids
        };

        // Sanitize character IDs to valid ranges as an invalid ID causes entire affiliation request to fail
        let character_ids: Vec<i64> = character_ids
            .into_iter()
            .filter(|&id| {
                let valid = is_valid_character_id(id);
                if !valid {
                    tracing::warn!(
                        character_id = id,
                        "Encountered invalid character ID while updating affiliations; skipping character"
                    );
                }
                valid
            })
            .collect();

        if character_ids.is_empty() {
            return Ok(());
        }

        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry("affiliation update", |cache| {
            let db = db.clone();
            let esi_client = esi_client.clone();
            let character_ids = character_ids.clone();

            Box::pin(async move {
                // Fetch affiliation data from ESI
                let affiliations = esi_client
                    .character()
                    .character_affiliation(character_ids)
                    .send()
                    .await?
                    .data;

                if affiliations.is_empty() {
                    return Ok(());
                }

                // Extract unique IDs from affiliations
                let character_ids: Vec<i64> = affiliations
                    .iter()
                    .map(|a| a.character_id)
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();
                let corporation_ids: Vec<i64> = affiliations
                    .iter()
                    .map(|a| a.corporation_id)
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();
                let alliance_ids: Vec<i64> = affiliations
                    .iter()
                    .filter_map(|a| a.alliance_id)
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();
                let faction_ids: Vec<i64> = affiliations
                    .iter()
                    .filter_map(|a| a.faction_id)
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();

                let faction_orch = FactionOrchestrator::new(&db, &esi_client);
                let alliance_orch = AllianceOrchestrator::new(&db, &esi_client);
                let corporation_orch = CorporationOrchestrator::new(&db, &esi_client);
                let character_orch = CharacterOrchestrator::new(&db, &esi_client);

                // Ensure all entities exist in dependency order
                // This will check the database first and only fetch missing entities from ESI
                if !faction_ids.is_empty() {
                    tracing::debug!("Ensuring {} factions exist", faction_ids.len());
                    faction_orch
                        .ensure_factions_exist(faction_ids, cache)
                        .await?;
                }

                if !alliance_ids.is_empty() {
                    tracing::debug!("Ensuring {} alliances exist", alliance_ids.len());
                    alliance_orch
                        .ensure_alliances_exist(alliance_ids, cache)
                        .await?;
                }

                if !corporation_ids.is_empty() {
                    tracing::debug!("Ensuring {} corporations exist", corporation_ids.len());
                    corporation_orch
                        .ensure_corporations_exist(corporation_ids, cache)
                        .await?;
                }

                if !character_ids.is_empty() {
                    tracing::debug!("Ensuring {} characters exist", character_ids.len());
                    character_orch
                        .ensure_characters_exist(character_ids, cache)
                        .await?;
                }

                // Persist all fetched entities and update affiliations in a transaction
                let txn = TrackedTransaction::begin(&db).await?;

                // Persist all entities from cache in dependency order
                cache.persist_all(&db, &esi_client, &txn).await?;

                // Update the affiliation relationships
                Self::update_affiliation_relationships(txn.as_ref(), &affiliations, cache).await?;

                txn.commit().await?;

                Ok(())
            })
        })
        .await
    }

    /// Updates affiliation relationships in the database.
    ///
    /// Processes ESI affiliation data and updates the database relationships between characters,
    /// corporations, and alliances. Uses the orchestration cache to map EVE IDs to internal
    /// database record IDs. Logs warnings for any missing IDs in the cache.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to execute updates within
    /// - `affiliations` - ESI affiliation data containing character/corporation/alliance/faction relationships
    /// - `cache` - Orchestration cache with ID mappings from EVE IDs to database record IDs
    ///
    /// # Returns
    /// - `Ok(())` - All relationship updates completed successfully
    /// - `Err(Error::DbErr)` - Database update operation failed
    async fn update_affiliation_relationships(
        txn: &sea_orm::DatabaseTransaction,
        affiliations: &[CharacterAffiliation],
        cache: &OrchestrationCache,
    ) -> Result<(), Error> {
        // Update corporation affiliations (corporation -> alliance)
        let corporation_updates: Vec<(i64, Option<i64>)> = affiliations
            .iter()
            .map(|a| (a.corporation_id, a.alliance_id))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let corporation_db_updates: Vec<(i32, Option<i32>)> = corporation_updates
            .into_iter()
            .filter_map(|(corp_id, alliance_id)| {
                let corp_db_id = cache.corporation_db_id.get(&corp_id).copied();
                if corp_db_id.is_none() {
                    tracing::warn!(
                        corporation_id = corp_id,
                        "Corporation ID not found in cache; skipping corporation affiliation update"
                    );
                    return None;
                }

                let alliance_db_id = match alliance_id {
                    Some(alliance_id) => {
                        let db_id = cache.alliance_db_id.get(&alliance_id).copied();
                        if db_id.is_none() {
                            tracing::warn!(
                                corporation_id = corp_id,
                                alliance_id = alliance_id,
                                "Alliance ID not found in cache; skipping corporation affiliation update"
                            );
                            return None;
                        }
                        db_id
                    }
                    None => None,
                };

                Some((corp_db_id.unwrap(), alliance_db_id))
            })
            .collect();

        if !corporation_db_updates.is_empty() {
            CorporationRepository::new(txn)
                .update_affiliations(corporation_db_updates)
                .await?;
        }

        // Update character affiliations (character -> corporation, faction)
        let character_db_updates: Vec<(i32, i32, Option<i32>)> = affiliations
            .iter()
            .filter_map(|a| {
                let char_db_id = cache.character_db_id.get(&a.character_id).copied();
                if char_db_id.is_none() {
                    tracing::warn!(
                        character_id = a.character_id,
                        "Character ID not found in cache; skipping character affiliation update"
                    );
                    return None;
                }

                let corp_db_id = cache.corporation_db_id.get(&a.corporation_id).copied();
                if corp_db_id.is_none() {
                    tracing::warn!(
                        character_id = a.character_id,
                        corporation_id = a.corporation_id,
                        "Corporation ID not found in cache; skipping character affiliation update"
                    );
                    return None;
                }

                let faction_db_id = match a.faction_id {
                    Some(faction_id) => {
                        let db_id = cache.faction_db_id.get(&faction_id).copied();
                        if db_id.is_none() {
                            tracing::warn!(
                                character_id = a.character_id,
                                faction_id = faction_id,
                                "Faction ID not found in cache; setting character's faction to None"
                            );
                        }
                        db_id
                    }
                    None => None,
                };

                Some((char_db_id.unwrap(), corp_db_id.unwrap(), faction_db_id))
            })
            .collect();

        if !character_db_updates.is_empty() {
            CharacterRepository::new(txn)
                .update_affiliations(character_db_updates)
                .await?;
        }

        Ok(())
    }
}
