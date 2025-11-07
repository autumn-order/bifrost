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

pub struct AffiliationService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> AffiliationService<'a> {
    pub async fn update_affiliations(&self, character_ids: Vec<i64>) -> Result<(), Error> {
        // TODO: sanitize character IDs to acceptable ID ranges before affiliations request

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

        let faction_table_ids = FactionRepository::new(&self.db)
            .get_entry_ids_by_faction_ids(&unique_faction_ids)
            .await?;
        let alliance_table_ids = AllianceRepository::new(&self.db)
            .get_entry_ids_by_alliance_ids(&unique_alliance_ids)
            .await?;
        let corporation_table_ids = CorporationRepository::new(&self.db)
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

    // Updates a corporation's affiliated alliance
    async fn update_corporation_affiliations(
        &self,
        affiliations: &[CharacterAffiliation],
        table_ids: &TableIds,
    ) -> Result<(), Error> {
        let corporation_affiliations: Vec<(i32, Option<i32>)> = affiliations.iter()
            .map(|a| (a.corporation_id, a.alliance_id))
            .collect::<HashSet<_>>() // Deduplicate
            .into_iter()
            .filter_map(|(corporation_id, alliance_id)| {
                let corporation_table_id = table_ids.corporation_ids
                    .get(&corporation_id)
                    .copied();

                // Skip if corporation not found
                let corporation_table_id = match corporation_table_id {
                    Some(id) => id,
                    None => {
                        tracing::warn!(
                            corporation_id = corporation_id,
                            "Corporation's ID not found in database; skipping corporation affiliation update"
                        );
                        return None;
                    }
                };

                let alliance_table_id = match alliance_id {
                    Some(alliance_id) => {
                        let alliance_table_id = table_ids.alliance_ids
                            .get(&alliance_id)
                            .copied();

                        // Skip if alliance not found
                        match alliance_table_id {
                            Some(id) => Some(id),
                            None => {
                                tracing::warn!(
                                    corporation_id = corporation_id,
                                    alliance_id = alliance_id,
                                    "Corporation's alliance ID not found in database; skipping corporation affiliation update"
                                );
                                return None;
                            }
                        }
                    }
                    None => None,
                };

                Some((corporation_table_id, alliance_table_id))
            })
            .collect();

        Ok(())
    }

    // Updates a character's affiliated corporation & faction
    async fn update_character_affiliations(
        &self,
        affiliations: &[CharacterAffiliation],
        table_ids: &TableIds,
    ) -> Result<(), Error> {
        let character_affiliations: Vec<(i32, i32, Option<i32>)> = affiliations
            .iter()
            .map(|a| (a.character_id, a.corporation_id, a.faction_id))
            .collect::<HashSet<_>>() // Deduplicate
            .into_iter()
            .filter_map(
                |(character_id, corporation_id, faction_id)| {
                    let character_table_id = table_ids.character_ids
                        .get(&character_id)
                        .copied();

                    let corporation_table_id = table_ids.corporation_ids
                        .get(&corporation_id)
                        .copied();

                    // Skip if character not found
                    let character_table_id = match character_table_id {
                        Some(id) => id,
                        None => {
                            tracing::warn!(
                                character_id = character_id,
                                corporation_id = corporation_id,
                                "Character's ID not found in database; skipping character affiliation update"
                            );
                            return None;
                        }
                    };

                    // Skip if corporation not found
                    let corporation_table_id = match corporation_table_id {
                        Some(id) => id,
                        None => {
                            tracing::warn!(
                                character_id = character_id,
                                corporation_id = corporation_id,
                                "Character's corporation ID not found in database; skipping character affiliation update"
                            );
                            return None;
                        }
                    };

                    let faction_table_id = match faction_id {
                        Some(faction_id) => {
                            let faction_table_id = table_ids.faction_ids
                                .get(&faction_id)
                                .copied();

                            // Set faction to None if faction is not found
                            match faction_table_id {
                                Some(id) => Some(id),
                                None => {
                                    tracing::warn!(
                                        character_id = character_id,
                                        faction_id = faction_id,
                                        "Character's faction ID not found in database; character's faction will be set as none for now"
                                    );
                                    None
                                }
                            }
                        }
                        None => None,
                    };

                    Some((character_table_id, corporation_table_id, faction_table_id))
                }
            ).collect();

        Ok(())
    }

    async fn fetch_missing_characters(
        &self,
        table_ids: &mut TableIds,
        unique_ids: &mut UniqueIds,
    ) -> Result<Vec<(i64, Character)>, Error> {
        let unique_character_ids: Vec<i64> = unique_ids.character_ids.iter().copied().collect();
        let missing_character_ids: Vec<i64> = unique_character_ids
            .into_iter()
            .filter(|id| !table_ids.character_ids.contains_key(id))
            .collect();

        if missing_character_ids.is_empty() {
            return Ok(Vec::new());
        }

        let fetched_characters = CharacterService::new(&self.db, &self.esi_client)
            .get_many_characters(missing_character_ids)
            .await?;

        for (_, character) in &fetched_characters {
            unique_ids.corporation_ids.insert(character.corporation_id);

            if let Some(faction_id) = character.faction_id {
                unique_ids.faction_ids.insert(faction_id);
            }
        }

        Ok(fetched_characters)
    }

    async fn fetch_missing_corporations(
        &self,
        table_ids: &mut TableIds,
        unique_ids: &mut UniqueIds,
    ) -> Result<Vec<(i64, Corporation)>, Error> {
        let unique_corporation_ids: Vec<i64> = unique_ids.corporation_ids.iter().copied().collect();
        let missing_corporation_ids: Vec<i64> = unique_corporation_ids
            .into_iter()
            .filter(|id| !table_ids.corporation_ids.contains_key(id))
            .collect();

        if missing_corporation_ids.is_empty() {
            return Ok(Vec::new());
        }

        let fetched_corporations = CorporationService::new(&self.db, &self.esi_client)
            .get_many_corporations(missing_corporation_ids)
            .await?;

        for (_, corporation) in &fetched_corporations {
            if let Some(alliance_id) = corporation.alliance_id {
                unique_ids.alliance_ids.insert(alliance_id);
            }
            if let Some(faction_id) = corporation.faction_id {
                unique_ids.faction_ids.insert(faction_id);
            }
        }

        Ok(fetched_corporations)
    }

    async fn fetch_missing_alliances(
        &self,
        table_ids: &mut TableIds,
        unique_ids: &mut UniqueIds,
    ) -> Result<Vec<(i64, Alliance)>, Error> {
        let unique_alliance_ids: Vec<i64> = unique_ids.alliance_ids.iter().copied().collect();
        let missing_alliance_ids: Vec<i64> = unique_alliance_ids
            .into_iter()
            .filter(|id| !table_ids.alliance_ids.contains_key(id))
            .collect();

        if missing_alliance_ids.is_empty() {
            return Ok(Vec::new());
        }

        let fetched_alliances = AllianceService::new(&self.db, &self.esi_client)
            .get_many_alliances(missing_alliance_ids)
            .await?;

        for (_, alliance) in &fetched_alliances {
            if let Some(faction_id) = alliance.faction_id {
                unique_ids.faction_ids.insert(faction_id);
            }
        }

        Ok(fetched_alliances)
    }

    async fn attempt_update_missing_factions(
        &self,
        table_ids: &mut TableIds,
        unique_ids: &mut UniqueIds,
    ) -> Result<(), Error> {
        let unique_faction_ids: Vec<i64> = unique_ids.faction_ids.iter().copied().collect();
        let missing_faction_ids: Vec<i64> = unique_faction_ids
            .into_iter()
            .filter(|id| !table_ids.faction_ids.contains_key(id))
            .collect();

        if missing_faction_ids.is_empty() {
            return Ok(());
        }

        let updated_factions = FactionService::new(&self.db, &self.esi_client)
            .update_factions()
            .await?;

        let updated_faction_ids: Vec<i64> = updated_factions.iter().map(|f| f.faction_id).collect();

        if !updated_faction_ids.is_empty() {
            let unique_faction_ids: Vec<i64> = unique_ids.faction_ids.iter().copied().collect();

            let faction_table_ids = FactionRepository::new(&self.db)
                .get_entry_ids_by_faction_ids(&unique_faction_ids)
                .await?;

            table_ids.faction_ids = faction_table_ids
                .iter()
                .map(|(table_id, faction_id)| (*faction_id, *table_id))
                .collect()
        }

        Ok(())
    }

    async fn store_fetched_characters(
        &self,
        fetched_characters: Vec<(i64, Character)>,
        table_ids: &TableIds,
    ) -> Result<(), Error> {
        // Insert fetched characters
        let character_entries: Vec<(i64, Character, i32, Option<i32>)> = fetched_characters
            .into_iter()
            .filter_map(|(character_id, character)| {
                let corporation_table_id = table_ids.corporation_ids
                    .get(&character.corporation_id)
                    .copied();

                // Skip if corporation not found
                let corporation_table_id = match corporation_table_id {
                    Some(id) => id,
                    None => {
                        tracing::warn!(
                            character_id = character_id,
                            corporation_id = character.corporation_id,
                            "Character's corporation ID not found in database; skipping character creation"
                        );
                        return None;
                    }
                };

                let faction_table_id = character
                    .faction_id
                    .and_then(|faction_id| table_ids.faction_ids.get(&faction_id).copied());

                Some((
                    character_id,
                    character,
                    corporation_table_id,
                    faction_table_id,
                ))
            })
            .collect();
        CharacterRepository::new(&self.db)
            .upsert_many(character_entries)
            .await?;

        Ok(())
    }

    async fn store_fetched_corporations(
        &self,
        fetched_corporations: Vec<(i64, Corporation)>,
        table_ids: &mut TableIds,
    ) -> Result<(), Error> {
        let corporation_entries: Vec<(i64, Corporation, Option<i32>, Option<i32>)> =
            fetched_corporations
                .into_iter()
                .map(|(corporation_id, corporation)| {
                    let alliance_table_id = corporation
                        .alliance_id
                        .and_then(|alliance_id| table_ids.alliance_ids.get(&alliance_id).copied());

                    let faction_table_id = corporation
                        .faction_id
                        .and_then(|faction_id| table_ids.faction_ids.get(&faction_id).copied());

                    (
                        corporation_id,
                        corporation,
                        alliance_table_id,
                        faction_table_id,
                    )
                })
                .collect();
        let created_corporations = CorporationRepository::new(&self.db)
            .upsert_many(corporation_entries)
            .await?;

        for corporation in created_corporations {
            table_ids
                .corporation_ids
                .insert(corporation.corporation_id, corporation.id);
        }

        Ok(())
    }

    async fn store_fetched_alliances(
        &self,
        fetched_alliances: Vec<(i64, Alliance)>,
        table_ids: &mut TableIds,
    ) -> Result<(), Error> {
        let alliance_entries: Vec<(i64, Alliance, Option<i32>)> = fetched_alliances
            .into_iter()
            .map(|(alliance_id, alliance)| {
                let faction_table_id = alliance
                    .faction_id
                    .and_then(|faction_id| table_ids.faction_ids.get(&faction_id).copied());

                (alliance_id, alliance, faction_table_id)
            })
            .collect();
        let created_alliances = AllianceRepository::new(&self.db)
            .upsert_many(alliance_entries)
            .await?;

        for alliance in created_alliances {
            table_ids
                .alliance_ids
                .insert(alliance.alliance_id, alliance.id);
        }

        Ok(())
    }
}

fn get_missing_ids(all_ids: &[i64], existing_ids: &[i64]) -> Vec<i64> {
    all_ids
        .iter()
        .filter(|id| !existing_ids.contains(id))
        .copied()
        .collect()
}
