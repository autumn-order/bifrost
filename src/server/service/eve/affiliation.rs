use std::collections::HashSet;

use dioxus_logger::tracing;
use eve_esi::model::character::CharacterAffiliation;
use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::server::{
    data::eve::{character::CharacterRepository, corporation::CorporationRepository},
    error::Error,
    service::{
        orchestrator::{
            alliance::AllianceOrchestrator, character::CharacterOrchestrator,
            corporation::CorporationOrchestrator, faction::FactionOrchestrator, OrchestrationCache,
        },
        retry::RetryContext,
    },
    util::eve::{is_valid_character_id, ESI_AFFILIATION_REQUEST_LIMIT},
};

pub struct AffiliationService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> AffiliationService<'a> {
    /// Creates a new instance of [`AffiliationService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Update character and corporation affiliations based on ESI affiliation data
    ///
    /// This method fetches character affiliation data from ESI and only fetches missing
    /// FK dependencies from ESI to ensure foreign key constraints are satisfied.
    ///
    /// # Arguments
    /// - `character_ids` - List of character IDs to update affiliations for
    ///
    /// # Returns
    /// - `Ok(())` - If all affiliations were successfully updated
    /// - `Err(Error)` - If any error occurred during the process
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
                    .await?;

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

                // Reset persistence flags before transaction attempt
                cache.reset_persistence_flags();

                // Persist all fetched entities and update affiliations in a transaction
                let txn = db.begin().await?;

                // Persist all entities from cache in dependency order
                cache.persist_all(&db, &esi_client, &txn).await?;

                // Update the affiliation relationships
                Self::update_affiliation_relationships(&txn, &affiliations, cache).await?;

                txn.commit().await?;

                Ok(())
            })
        })
        .await
    }

    /// Update the affiliation relationships between characters, corporations, and alliances
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
