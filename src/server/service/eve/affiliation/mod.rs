#[cfg(test)]
mod tests;

mod fetch;
mod store;
mod update;

use std::collections::{HashMap, HashSet};

use dioxus_logger::tracing;
use eve_esi::model::{
    alliance::Alliance,
    character::{Character, CharacterAffiliation},
    corporation::Corporation,
};
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::{
        alliance::AllianceRepository, character::CharacterRepository,
        corporation::CorporationRepository, faction::FactionRepository,
    },
    error::Error,
    service::eve::{
        alliance::AllianceService, character::CharacterService, corporation::CorporationService,
        faction::FactionService,
    },
    util::eve::{is_valid_character_id, ESI_AFFILIATION_REQUEST_LIMIT},
};

struct TableIds {
    faction_ids: HashMap<i64, i32>,
    alliance_ids: HashMap<i64, i32>,
    corporation_ids: HashMap<i64, i32>,
    character_ids: HashMap<i64, i32>,
}

struct UniqueIds {
    faction_ids: HashSet<i64>,
    alliance_ids: HashSet<i64>,
    corporation_ids: HashSet<i64>,
    character_ids: HashSet<i64>,
}

pub struct AffiliationService {
    db: DatabaseConnection,
    esi_client: eve_esi::Client,
}

impl AffiliationService {
    /// Creates a new instance of [`AffiliationService`]
    pub fn new(db: DatabaseConnection, esi_client: eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    pub async fn update_affiliations(&self, character_ids: Vec<i64>) -> Result<(), Error> {
        // Cap character_ids to ESI limit to prevent affiliation request from erroring
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
        let character_ids = character_ids
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

        let affiliations = self
            .esi_client
            .character()
            .character_affiliation(character_ids)
            .await?;

        let mut unique_ids = UniqueIds {
            faction_ids: affiliations
                .iter()
                .filter_map(|a| a.faction_id)
                .collect::<HashSet<_>>(),
            alliance_ids: affiliations
                .iter()
                .filter_map(|a| a.alliance_id)
                .collect::<HashSet<_>>(),
            corporation_ids: affiliations
                .iter()
                .map(|a| a.corporation_id)
                .collect::<HashSet<_>>(),
            character_ids: affiliations
                .iter()
                .map(|a| a.character_id)
                .collect::<HashSet<_>>(),
        };

        let mut table_ids = self.find_existing_entity_ids(&unique_ids).await?;

        self.fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
            .await?;

        self.update_corporation_affiliations(&affiliations, &table_ids)
            .await?;
        self.update_character_affiliations(&affiliations, &table_ids)
            .await?;

        Ok(())
    }

    // Get the table ID (i32) and EVE Online ID (i64) for each entity in affiliations
    async fn find_existing_entity_ids(&self, unique_ids: &UniqueIds) -> Result<TableIds, Error> {
        let unique_faction_ids: Vec<i64> = unique_ids.faction_ids.iter().copied().collect();
        let unique_alliance_ids: Vec<i64> = unique_ids.alliance_ids.iter().copied().collect();
        let unique_corporation_ids: Vec<i64> = unique_ids.corporation_ids.iter().copied().collect();
        let unique_character_ids: Vec<i64> = unique_ids.character_ids.iter().copied().collect();

        let faction_table_ids = FactionRepository::new(self.db.clone())
            .get_entry_ids_by_faction_ids(&unique_faction_ids)
            .await?;
        let alliance_table_ids = AllianceRepository::new(self.db.clone())
            .get_entry_ids_by_alliance_ids(&unique_alliance_ids)
            .await?;
        let corporation_table_ids = CorporationRepository::new(self.db.clone())
            .get_entry_ids_by_corporation_ids(&unique_corporation_ids)
            .await?;
        let character_table_ids = CharacterRepository::new(&self.db)
            .get_entry_ids_by_character_ids(&unique_character_ids)
            .await?;

        Ok(TableIds {
            faction_ids: faction_table_ids
                .iter()
                .map(|(table_id, faction_id)| (*faction_id, *table_id))
                .collect(),
            alliance_ids: alliance_table_ids
                .iter()
                .map(|(table_id, alliance_id)| (*alliance_id, *table_id))
                .collect(),
            corporation_ids: corporation_table_ids
                .iter()
                .map(|(table_id, corporation_id)| (*corporation_id, *table_id))
                .collect(),
            character_ids: character_table_ids
                .iter()
                .map(|(table_id, character_id)| (*character_id, *table_id))
                .collect(),
        })
    }

    // Retrieve any missing enitites from ESI & inserts to database
    async fn fetch_and_store_missing_entities(
        &self,
        mut table_ids: &mut TableIds,
        mut unique_ids: &mut UniqueIds,
    ) -> Result<(), Error> {
        let fetched_alliances = self
            .fetch_missing_alliances(table_ids, &mut unique_ids)
            .await?;
        let fetched_corporations = self
            .fetch_missing_corporations(table_ids, &mut unique_ids)
            .await?;
        let fetched_characters = self
            .fetch_missing_characters(table_ids, &mut unique_ids)
            .await?;

        self.attempt_update_missing_factions(&mut table_ids, &mut unique_ids)
            .await?;

        if !fetched_alliances.is_empty() {
            self.store_fetched_alliances(fetched_alliances, &mut table_ids)
                .await?;
        }
        if !fetched_corporations.is_empty() {
            self.store_fetched_corporations(fetched_corporations, &mut table_ids)
                .await?;
        }
        if !fetched_characters.is_empty() {
            self.store_fetched_characters(fetched_characters, &mut table_ids)
                .await?;
        }

        Ok(())
    }
}
