//! Affiliation service for EVE Online character and corporation affiliation updates.
//!
//! This module provides the `AffiliationService` for bulk updating character and corporation
//! affiliations from ESI. It handles fetching affiliation data, resolving dependencies,
//! and updating relationships in a single transaction.

use std::collections::HashSet;

use dioxus_logger::tracing;
use eve_esi::model::character::CharacterAffiliation;
use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::server::{
    data::eve::{character::CharacterRepository, corporation::CorporationRepository},
    error::Error,
    service::eve::provider::{EveEntityProvider, StoredEntities},
    util::eve::{is_valid_character_id, ESI_AFFILIATION_REQUEST_LIMIT},
};

/// Service for managing EVE Online affiliation updates.
///
/// Provides methods for bulk updating character and corporation affiliations from ESI.
/// Handles dependency resolution for factions, alliances, and corporations, and updates
/// all relationships in a single transaction.
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
    /// dependencies (factions, alliances, corporations, characters) to satisfy foreign key constraints.
    /// All updates are performed in a single transaction.
    ///
    /// The method validates and sanitizes input character IDs, caps the request to ESI limits,
    /// and skips invalid IDs to prevent request failures.
    ///
    /// For efficiency, this method only fetches entities from ESI that don't already exist in the
    /// database, making it suitable for bulk operations with up to 1000 characters.
    ///
    /// # Arguments
    /// - `character_ids` - List of EVE character IDs to update affiliations for (max 1000)
    ///
    /// # Returns
    /// - `Ok(())` - All affiliations successfully updated
    /// - `Err(Error::EsiError)` - Failed to fetch affiliation or dependency data from ESI
    /// - `Err(Error::DbErr)` - Database operation failed
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

        // Fetch affiliation data from ESI
        let affiliations = self
            .esi_client
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

        // Build entity provider with all entities needed for affiliations
        // Using ensure_*_exist methods to only fetch entities missing from database
        tracing::debug!(
            "Ensuring {} characters, {} corporations, {} alliances, {} factions exist",
            character_ids.len(),
            corporation_ids.len(),
            alliance_ids.len(),
            faction_ids.len()
        );

        let eve_entity_provider = EveEntityProvider::builder(self.db, self.esi_client)
            .ensure_characters_exist(character_ids.clone())
            .ensure_corporations_exist(corporation_ids.clone())
            .ensure_alliances_exist(alliance_ids.clone())
            .ensure_factions_exist(faction_ids.clone())
            .build()
            .await?;

        // Persist any newly fetched entities in a transaction
        let txn = self.db.begin().await?;
        let stored_entities = eve_entity_provider.store(&txn).await?;

        // Update the affiliation relationships
        Self::update_affiliation_relationships(&txn, &affiliations, stored_entities).await?;

        txn.commit().await?;

        Ok(())
    }

    /// Updates affiliation relationships in the database.
    ///
    /// Processes ESI affiliation data and updates the database relationships between characters,
    /// corporations, and alliances. Uses the entity record IDs to map EVE IDs to internal
    /// database record IDs. Logs warnings for any missing IDs.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to execute updates within
    /// - `affiliations` - ESI affiliation data containing character/corporation/alliance/faction relationships
    /// - `entity_record_ids` - Maps of EVE IDs to database record IDs
    ///
    /// # Returns
    /// - `Ok(())` - All relationship updates completed successfully
    /// - `Err(Error::DbErr)` - Database update operation failed
    async fn update_affiliation_relationships(
        txn: &sea_orm::DatabaseTransaction,
        affiliations: &[CharacterAffiliation],
        stored_entities: StoredEntities,
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
                let Some(corp_record_id) = stored_entities.get_corporation_record_id(&corp_id) else {
                    tracing::warn!(
                        corporation_id = corp_id,
                        "Corporation ID not found in database; skipping corporation affiliation update"
                    );
                    return None;
                };

                let alliance_db_id = match alliance_id {
                    Some(alliance_id) => {
                        let db_id = stored_entities.get_alliance_record_id(&alliance_id);
                        if db_id.is_none() {
                            tracing::warn!(
                                corporation_id = corp_id,
                                alliance_id = alliance_id,
                                "Alliance ID not found in database; skipping corporation affiliation update"
                            );
                            return None;
                        }
                        Some(db_id.unwrap())
                    }
                    None => None,
                };

                Some((corp_record_id, alliance_db_id))
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
                let Some(char_db_id) = stored_entities.get_character_record_id(&a.character_id) else {
                    tracing::warn!(
                        character_id = a.character_id,
                        "Character ID not found in database; skipping character affiliation update"
                    );
                    return None;
                };

                let Some(corp_db_id) = stored_entities.get_corporation_record_id(&a.corporation_id) else {
                    tracing::warn!(
                        character_id = a.character_id,
                        corporation_id = a.corporation_id,
                        "Corporation ID not found in database; skipping character affiliation update"
                    );
                    return None;
                };

                let faction_db_id = match a.faction_id {
                    Some(faction_id) => {
                        let db_id = stored_entities.get_faction_record_id(&faction_id);
                        if db_id.is_none() {
                            tracing::warn!(
                                character_id = a.character_id,
                                faction_id = faction_id,
                                "Faction ID not found in database; setting character's faction to None"
                            );
                        }
                        db_id
                    }
                    None => None,
                };

                Some((char_db_id, corp_db_id, faction_db_id))
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
