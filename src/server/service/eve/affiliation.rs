use std::collections::{HashMap, HashSet};

use chrono::Utc;
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

pub struct AffiliationService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> AffiliationService<'a> {
    /// Creates a new instance of [`AffiliationService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    pub async fn update_affiliations(&self, character_ids: Vec<i64>) -> Result<(), Error> {
        let character_repo = CharacterRepository::new(&self.db);
        let corporation_repo = CorporationRepository::new(&self.db);
        let alliance_repo = AllianceRepository::new(&self.db);
        let faction_repo = FactionRepository::new(&self.db);

        // If the database were to have any invalid IDs inserted in the character table, this will fail
        // for the entirety of the provided character IDs. No sanitization is added for IDs as all IDs
        // present in the database *should* be directly from ESI.
        //
        // Unfortunately the error won't actually tell you which provided ID caused the error. We'll add
        // character ID sanitization later as a shared utility if this proves to be an issue.
        //
        // Deleted characters will still be returned but as members of the Doomheim corporation id `1000001`
        let affiliations = self
            .esi_client
            .character()
            .character_affiliation(character_ids)
            .await?;

        // Create HashSets of all IDs to ensure we only retrieve unique IDs
        let character_ids: HashSet<i64> = affiliations.iter().map(|a| a.character_id).collect();
        let mut corporation_ids: HashSet<i64> =
            affiliations.iter().map(|a| a.corporation_id).collect();
        let mut alliance_ids: HashSet<i64> =
            affiliations.iter().filter_map(|a| a.alliance_id).collect();
        let mut faction_ids: HashSet<i64> =
            affiliations.iter().filter_map(|a| a.faction_id).collect();

        // Get all faction, alliance, and corporation IDs present in the database
        let faction_ids_vec: Vec<i64> = faction_ids.iter().copied().collect();
        let alliance_ids_vec: Vec<i64> = alliance_ids.iter().copied().collect();
        let corporation_ids_vec: Vec<i64> = corporation_ids.iter().copied().collect();
        let character_ids_vec: Vec<i64> = character_ids.iter().copied().collect();

        let mut faction_table_ids = faction_repo
            .get_entry_ids_by_faction_ids(&faction_ids_vec)
            .await?;
        let alliance_table_ids = alliance_repo
            .get_entry_ids_by_alliance_ids(&alliance_ids_vec)
            .await?;
        let corporation_table_ids = corporation_repo
            .get_entry_ids_by_corporation_ids(&corporation_ids_vec)
            .await?;
        let character_table_ids = character_repo
            .get_entry_ids_by_character_ids(&character_ids_vec)
            .await?;

        let existing_character_ids: Vec<i64> = character_table_ids
            .iter()
            .map(|(_, character_id)| *character_id)
            .collect();
        let existing_corporation_ids: Vec<i64> = corporation_table_ids
            .iter()
            .map(|(_, corporation_id)| *corporation_id)
            .collect();
        let existing_alliance_ids: Vec<i64> = alliance_table_ids
            .iter()
            .map(|(_, alliance_id)| *alliance_id)
            .collect();
        let existing_faction_ids: Vec<i64> = faction_table_ids
            .iter()
            .map(|(_, faction_id)| *faction_id)
            .collect();

        // Fetch any missing characters that are missing from database
        let fetched_characters = self
            .fetch_missing_characters(&character_ids_vec, &existing_character_ids)
            .await?;

        // From the fetched characters, insert any missing corporation/factions to ID list
        for (_, character) in &fetched_characters {
            corporation_ids.insert(character.corporation_id);

            if let Some(faction_id) = character.faction_id {
                faction_ids.insert(faction_id);
            }
        }

        // Fetch any corporations that are missing from database
        let fetched_corporations = self
            .fetch_missing_corporations(&corporation_ids_vec, &existing_corporation_ids)
            .await?;

        // From the fetched corporations, insert any missing alliances/factions to ID list
        for (_, corporation) in &fetched_corporations {
            if let Some(alliance_id) = corporation.alliance_id {
                alliance_ids.insert(alliance_id);
            }
            if let Some(faction_id) = corporation.faction_id {
                faction_ids.insert(faction_id);
            }
        }

        // Fetch any alliances that are missing from database
        let fetched_alliances = self
            .fetch_missing_alliances(&alliance_ids_vec, &existing_alliance_ids)
            .await?;

        // From the fetched alliances, insert any missing factions to ID list
        for (_, alliance) in &fetched_alliances {
            if let Some(faction_id) = alliance.faction_id {
                faction_ids.insert(faction_id);
            }
        }

        // Attempt to update factions if any are missing from database
        let updated_factions = self
            .fetch_missing_factions(&faction_ids_vec, &existing_faction_ids)
            .await?;

        let updated_faction_ids: Vec<i64> = updated_factions.iter().map(|f| f.faction_id).collect();

        // If factions were updated, re-fetch the faction table ID pairs
        if !updated_faction_ids.is_empty() {
            faction_table_ids = faction_repo
                .get_entry_ids_by_faction_ids(&faction_ids_vec)
                .await?;
        }

        // Create hashmaps to lookup faction, alliance, and corporation table IDs
        let faction_id_to_table_id: HashMap<i64, i32> = faction_table_ids
            .iter()
            .map(|(table_id, faction_id)| (*faction_id, *table_id))
            .collect();
        let mut alliance_id_to_table_id: HashMap<i64, i32> = alliance_table_ids
            .iter()
            .map(|(table_id, alliance_id)| (*alliance_id, *table_id))
            .collect();
        let mut corporation_id_to_table_id: HashMap<i64, i32> = corporation_table_ids
            .iter()
            .map(|(table_id, corporation_id)| (*corporation_id, *table_id))
            .collect();
        let character_id_to_table_id: HashMap<i64, i32> = character_table_ids
            .iter()
            .map(|(table_id, character_id)| (*character_id, *table_id))
            .collect();

        // Insert fetched alliances
        let alliance_entries: Vec<(i64, Alliance, Option<i32>)> = fetched_alliances
            .into_iter()
            .map(|(alliance_id, alliance)| {
                let faction_table_id = alliance
                    .faction_id
                    .and_then(|faction_id| faction_id_to_table_id.get(&faction_id).copied());

                (alliance_id, alliance, faction_table_id)
            })
            .collect();
        let created_alliances = alliance_repo.upsert_many(alliance_entries).await?;

        // Update alliance_id_to_table_id hashmap with newly created alliances
        for alliance in created_alliances {
            alliance_id_to_table_id.insert(alliance.alliance_id, alliance.id);
        }

        // Insert fetched corporations
        let corporation_entries: Vec<(i64, Corporation, Option<i32>, Option<i32>)> =
            fetched_corporations
                .into_iter()
                .map(|(corporation_id, corporation)| {
                    let alliance_table_id = corporation
                        .alliance_id
                        .and_then(|alliance_id| alliance_id_to_table_id.get(&alliance_id).copied());

                    let faction_table_id = corporation
                        .faction_id
                        .and_then(|faction_id| faction_id_to_table_id.get(&faction_id).copied());

                    (
                        corporation_id,
                        corporation,
                        alliance_table_id,
                        faction_table_id,
                    )
                })
                .collect();
        let created_corporations = corporation_repo.upsert_many(corporation_entries).await?;

        // Update corporation_id_to_table_id hashmap with newly created corporations
        for corporation in created_corporations {
            corporation_id_to_table_id.insert(corporation.corporation_id, corporation.id);
        }

        // Insert fetched characters
        let character_entries: Vec<(i64, Character, i32, Option<i32>)> = fetched_characters
            .into_iter()
            .filter_map(|(character_id, character)| {
                let corporation_table_id = corporation_id_to_table_id
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
                    .and_then(|faction_id| faction_id_to_table_id.get(&faction_id).copied());

                Some((
                    character_id,
                    character,
                    corporation_table_id,
                    faction_table_id,
                ))
            })
            .collect();
        character_repo.upsert_many(character_entries).await?;

        // Update only the corporation's alliance
        //
        // The corporation information update job handles the faction affiliation update
        let corporation_alliance_affiliations: Vec<(i32, Option<i32>)> = affiliations.iter()
            .map(|a| (a.corporation_id, a.alliance_id))
            .collect::<HashSet<_>>() // Deduplicate
            .into_iter()
            .filter_map(|(corporation_id, alliance_id)| {
                let corporation_table_id = corporation_id_to_table_id
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
                        let alliance_table_id = alliance_id_to_table_id
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

        let character_affiliations: Vec<(i32, i32, Option<i32>)> = affiliations
            .iter()
            .map(|a| (a.character_id, a.corporation_id, a.faction_id))
            .collect::<HashSet<_>>() // Deduplicate
            .into_iter()
            .filter_map(
                |(character_id, corporation_id, faction_id)| {
                    let character_table_id = character_id_to_table_id
                        .get(&character_id)
                        .copied();

                    let corporation_table_id = corporation_id_to_table_id
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
                            let faction_table_id = faction_id_to_table_id
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
        character_ids: &[i64],
        existing_character_ids: &[i64],
    ) -> Result<Vec<(i64, Character)>, Error> {
        let missing_ids = get_missing_ids(character_ids, existing_character_ids);

        if missing_ids.is_empty() {
            return Ok(Vec::new());
        }

        CharacterService::new(&self.db, &self.esi_client)
            .get_many_characters(missing_ids)
            .await
    }

    async fn fetch_missing_corporations(
        &self,
        corporation_ids: &[i64],
        existing_corporation_ids: &[i64],
    ) -> Result<Vec<(i64, Corporation)>, Error> {
        let missing_ids = get_missing_ids(corporation_ids, existing_corporation_ids);

        if missing_ids.is_empty() {
            return Ok(Vec::new());
        }

        CorporationService::new(&self.db, &self.esi_client)
            .get_many_corporations(missing_ids)
            .await
    }

    async fn fetch_missing_alliances(
        &self,
        alliance_ids: &[i64],
        existing_alliance_ids: &[i64],
    ) -> Result<Vec<(i64, Alliance)>, Error> {
        let missing_ids = get_missing_ids(alliance_ids, existing_alliance_ids);

        if missing_ids.is_empty() {
            return Ok(Vec::new());
        }

        AllianceService::new(&self.db, &self.esi_client)
            .get_many_alliances(missing_ids)
            .await
    }

    /// Fetches and stores information for any factions missing from affiliations
    ///
    /// If a faction isn't found even after an update, then the affiliation entry for
    /// the character's faction will be set as none for the time being.
    async fn fetch_missing_factions(
        &self,
        faction_ids: &[i64],
        existing_faction_ids: &[i64],
    ) -> Result<Vec<entity::eve_faction::Model>, Error> {
        let missing_ids = get_missing_ids(faction_ids, existing_faction_ids);

        if missing_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Fetch any factions, alliances, & corporations which don't exist from ESI and insert them into database
        // This should rarely occur unless an update had just happened which added a new faction.
        // In which case we should be able to retrieve it from ESI if we haven't already updated factions
        // since downtime at 11:05 EVE time
        //
        // This returns an empty array if factions stored are still within 24 hour cache period.
        FactionService::new(&self.db, &self.esi_client)
            .update_factions()
            .await
    }
}

fn get_missing_ids(all_ids: &[i64], existing_ids: &[i64]) -> Vec<i64> {
    all_ids
        .iter()
        .filter(|id| !existing_ids.contains(id))
        .copied()
        .collect()
}
