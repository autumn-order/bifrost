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
    util::eve::is_valid_character_id,
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

        CorporationRepository::new(&self.db)
            .update_affiliations(corporation_affiliations)
            .await?;

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

        CharacterRepository::new(&self.db)
            .update_affiliations(character_affiliations)
            .await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use bifrost_test_utils::{prelude::*, test_setup_with_tables};
    use std::collections::HashSet;

    mod find_existing_entity_ids {
        use super::*;

        /// Expect Ok with correct mappings when all entity types exist in database
        #[tokio::test]
        async fn returns_mappings_for_all_entity_types() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            // Insert test data
            let faction = test.eve().insert_mock_faction(500001).await?;
            let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let character = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;

            // Create service and input
            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let unique_ids = UniqueIds {
                faction_ids: vec![faction.faction_id].into_iter().collect(),
                alliance_ids: vec![alliance.alliance_id].into_iter().collect(),
                corporation_ids: vec![corporation.corporation_id].into_iter().collect(),
                character_ids: vec![character.character_id].into_iter().collect(),
            };

            // Execute
            let result = service.find_existing_entity_ids(&unique_ids).await;

            // Assert
            assert!(result.is_ok());
            let table_ids = result.unwrap();

            assert_eq!(table_ids.faction_ids.len(), 1);
            assert_eq!(table_ids.faction_ids[&faction.faction_id], faction.id);

            assert_eq!(table_ids.alliance_ids.len(), 1);
            assert_eq!(table_ids.alliance_ids[&alliance.alliance_id], alliance.id);

            assert_eq!(table_ids.corporation_ids.len(), 1);
            assert_eq!(
                table_ids.corporation_ids[&corporation.corporation_id],
                corporation.id
            );

            assert_eq!(table_ids.character_ids.len(), 1);
            assert_eq!(
                table_ids.character_ids[&character.character_id],
                character.id
            );

            Ok(())
        }

        /// Expect Ok with empty maps when no entities exist in database
        #[tokio::test]
        async fn returns_empty_when_no_entities_exist() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let unique_ids = UniqueIds {
                faction_ids: vec![500001, 500002].into_iter().collect(),
                alliance_ids: vec![99000001, 99000002].into_iter().collect(),
                corporation_ids: vec![98000001, 98000002].into_iter().collect(),
                character_ids: vec![2114794365, 2114794366].into_iter().collect(),
            };

            let result = service.find_existing_entity_ids(&unique_ids).await;

            assert!(result.is_ok());
            let table_ids = result.unwrap();

            assert_eq!(table_ids.faction_ids.len(), 0);
            assert_eq!(table_ids.alliance_ids.len(), 0);
            assert_eq!(table_ids.corporation_ids.len(), 0);
            assert_eq!(table_ids.character_ids.len(), 0);

            Ok(())
        }

        /// Expect Ok with empty maps when input is empty
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service.find_existing_entity_ids(&unique_ids).await;

            assert!(result.is_ok());
            let table_ids = result.unwrap();

            assert_eq!(table_ids.faction_ids.len(), 0);
            assert_eq!(table_ids.alliance_ids.len(), 0);
            assert_eq!(table_ids.corporation_ids.len(), 0);
            assert_eq!(table_ids.character_ids.len(), 0);

            Ok(())
        }

        /// Expect Ok with correct mappings when multiple entities of each type exist
        #[tokio::test]
        async fn returns_mappings_for_multiple_entities() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            // Insert multiple entities of each type
            let faction1 = test.eve().insert_mock_faction(500001).await?;
            let faction2 = test.eve().insert_mock_faction(500002).await?;
            let faction3 = test.eve().insert_mock_faction(500003).await?;

            let alliance1 = test.eve().insert_mock_alliance(99000001, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(99000002, None).await?;

            let corporation1 = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let corporation2 = test
                .eve()
                .insert_mock_corporation(98000002, None, None)
                .await?;
            let corporation3 = test
                .eve()
                .insert_mock_corporation(98000003, None, None)
                .await?;

            let character1 = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;
            let character2 = test
                .eve()
                .insert_mock_character(2114794366, 98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let unique_ids = UniqueIds {
                faction_ids: vec![
                    faction1.faction_id,
                    faction2.faction_id,
                    faction3.faction_id,
                ]
                .into_iter()
                .collect(),
                alliance_ids: vec![alliance1.alliance_id, alliance2.alliance_id]
                    .into_iter()
                    .collect(),
                corporation_ids: vec![
                    corporation1.corporation_id,
                    corporation2.corporation_id,
                    corporation3.corporation_id,
                ]
                .into_iter()
                .collect(),
                character_ids: vec![character1.character_id, character2.character_id]
                    .into_iter()
                    .collect(),
            };

            let result = service.find_existing_entity_ids(&unique_ids).await;

            assert!(result.is_ok());
            let table_ids = result.unwrap();

            // Verify correct counts
            assert_eq!(table_ids.faction_ids.len(), 3);
            assert_eq!(table_ids.alliance_ids.len(), 2);
            assert_eq!(table_ids.corporation_ids.len(), 3);
            assert_eq!(table_ids.character_ids.len(), 2);

            // Verify correct mappings
            assert_eq!(table_ids.faction_ids[&faction1.faction_id], faction1.id);
            assert_eq!(table_ids.faction_ids[&faction2.faction_id], faction2.id);
            assert_eq!(table_ids.faction_ids[&faction3.faction_id], faction3.id);

            assert_eq!(table_ids.alliance_ids[&alliance1.alliance_id], alliance1.id);
            assert_eq!(table_ids.alliance_ids[&alliance2.alliance_id], alliance2.id);

            assert_eq!(
                table_ids.corporation_ids[&corporation1.corporation_id],
                corporation1.id
            );
            assert_eq!(
                table_ids.corporation_ids[&corporation2.corporation_id],
                corporation2.id
            );
            assert_eq!(
                table_ids.corporation_ids[&corporation3.corporation_id],
                corporation3.id
            );

            assert_eq!(
                table_ids.character_ids[&character1.character_id],
                character1.id
            );
            assert_eq!(
                table_ids.character_ids[&character2.character_id],
                character2.id
            );

            Ok(())
        }

        /// Expect Ok with partial results when only some entities exist
        #[tokio::test]
        async fn returns_partial_results_for_mixed_input() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            // Insert only some entities
            let faction = test.eve().insert_mock_faction(500001).await?;
            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let unique_ids = UniqueIds {
                faction_ids: vec![500001, 500002, 500003].into_iter().collect(), // Only 500001 exists
                alliance_ids: vec![99000001, 99000002].into_iter().collect(),    // None exist
                corporation_ids: vec![98000001, 98000002].into_iter().collect(), // Only 98000001 exists
                character_ids: vec![2114794365].into_iter().collect(),           // None exist
            };

            let result = service.find_existing_entity_ids(&unique_ids).await;

            assert!(result.is_ok());
            let table_ids = result.unwrap();

            // Should only return the entities that exist
            assert_eq!(table_ids.faction_ids.len(), 1);
            assert_eq!(table_ids.faction_ids[&faction.faction_id], faction.id);

            assert_eq!(table_ids.alliance_ids.len(), 0);

            assert_eq!(table_ids.corporation_ids.len(), 1);
            assert_eq!(
                table_ids.corporation_ids[&corporation.corporation_id],
                corporation.id
            );

            assert_eq!(table_ids.character_ids.len(), 0);

            Ok(())
        }

        /// Expect Error when required tables haven't been created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?; // No tables created

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let unique_ids = UniqueIds {
                faction_ids: vec![500001].into_iter().collect(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service.find_existing_entity_ids(&unique_ids).await;

            assert!(result.is_err());

            Ok(())
        }

        /// Expect Ok with mappings for only the requested entity type
        #[tokio::test]
        async fn returns_mappings_for_single_entity_type() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let faction = test.eve().insert_mock_faction(500001).await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let unique_ids = UniqueIds {
                faction_ids: vec![faction.faction_id].into_iter().collect(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service.find_existing_entity_ids(&unique_ids).await;

            assert!(result.is_ok());
            let table_ids = result.unwrap();

            assert_eq!(table_ids.faction_ids.len(), 1);
            assert_eq!(table_ids.faction_ids[&faction.faction_id], faction.id);
            assert_eq!(table_ids.alliance_ids.len(), 0);
            assert_eq!(table_ids.corporation_ids.len(), 0);
            assert_eq!(table_ids.character_ids.len(), 0);

            Ok(())
        }

        /// Expect Ok with correct mapping direction from EVE ID to table ID
        #[tokio::test]
        async fn returns_correct_mapping_direction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let faction = test.eve().insert_mock_faction(500001).await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let unique_ids = UniqueIds {
                faction_ids: vec![faction.faction_id].into_iter().collect(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service.find_existing_entity_ids(&unique_ids).await;

            assert!(result.is_ok());
            let table_ids = result.unwrap();

            // The HashMap should map from EVE ID (i64) to table ID (i32)
            // faction.faction_id is the EVE ID
            // faction.id is the table ID
            assert!(table_ids.faction_ids.contains_key(&faction.faction_id));
            assert_eq!(table_ids.faction_ids[&faction.faction_id], faction.id);

            Ok(())
        }
    }

    mod fetch_missing_characters {
        use super::*;

        /// Expect Ok with fetched characters when characters are missing from database
        #[tokio::test]
        async fn fetches_missing_characters_from_esi() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            // Insert corporation so character can reference it
            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            // Setup mock ESI endpoints
            let (character_id, mock_character) = test
                .eve()
                .with_mock_character(2114794365, 98000001, None, None);
            let character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: vec![98000001].into_iter().collect(),
                character_ids: vec![character_id].into_iter().collect(),
            };

            let result = service
                .fetch_missing_characters(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            let fetched = result.unwrap();
            assert_eq!(fetched.len(), 1);
            assert_eq!(fetched[0].0, character_id);

            character_endpoint.assert();

            Ok(())
        }

        /// Expect Ok with empty vec when no characters are missing
        #[tokio::test]
        async fn returns_empty_when_no_characters_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let character = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: vec![(character.character_id, character.id)]
                    .into_iter()
                    .collect(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: vec![character.character_id].into_iter().collect(),
            };

            let result = service
                .fetch_missing_characters(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            let fetched = result.unwrap();
            assert_eq!(fetched.len(), 0);

            Ok(())
        }

        /// Expect Ok with empty vec when input is empty
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_missing_characters(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            let fetched = result.unwrap();
            assert_eq!(fetched.len(), 0);

            Ok(())
        }

        /// Expect Ok and verify corporation_ids are added to unique_ids
        #[tokio::test]
        async fn adds_corporation_ids_to_unique_ids() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let (character_id, mock_character) = test
                .eve()
                .with_mock_character(2114794365, 98000001, None, None);
            let _character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: vec![character_id].into_iter().collect(),
            };

            let result = service
                .fetch_missing_characters(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            assert!(unique_ids.corporation_ids.contains(&98000001));

            Ok(())
        }

        /// Expect Ok and verify faction_ids are added to unique_ids when present
        #[tokio::test]
        async fn adds_faction_ids_to_unique_ids_when_present() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let (character_id, mock_character) =
                test.eve()
                    .with_mock_character(2114794365, 98000001, None, Some(500001));
            let _character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: vec![character_id].into_iter().collect(),
            };

            let result = service
                .fetch_missing_characters(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            assert!(unique_ids.faction_ids.contains(&500001));

            Ok(())
        }
    }

    mod fetch_missing_corporations {
        use super::*;

        /// Expect Ok with fetched corporations when corporations are missing from database
        #[tokio::test]
        async fn fetches_missing_corporations_from_esi() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(98000001, None, None);
            let corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: vec![corporation_id].into_iter().collect(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_missing_corporations(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            let fetched = result.unwrap();
            assert_eq!(fetched.len(), 1);
            assert_eq!(fetched[0].0, corporation_id);

            corporation_endpoint.assert();

            Ok(())
        }

        /// Expect Ok with empty vec when no corporations are missing
        #[tokio::test]
        async fn returns_empty_when_no_corporations_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(corporation.corporation_id, corporation.id)]
                    .into_iter()
                    .collect(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: vec![corporation.corporation_id].into_iter().collect(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_missing_corporations(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            let fetched = result.unwrap();
            assert_eq!(fetched.len(), 0);

            Ok(())
        }

        /// Expect Ok with empty vec when input is empty
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_missing_corporations(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            let fetched = result.unwrap();
            assert_eq!(fetched.len(), 0);

            Ok(())
        }

        /// Expect Ok and verify alliance_ids are added to unique_ids when present
        #[tokio::test]
        async fn adds_alliance_ids_to_unique_ids_when_present() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (corporation_id, mock_corporation) =
                test.eve()
                    .with_mock_corporation(98000001, Some(99000001), None);
            let _corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: vec![corporation_id].into_iter().collect(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_missing_corporations(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            assert!(unique_ids.alliance_ids.contains(&99000001));

            Ok(())
        }

        /// Expect Ok and verify faction_ids are added to unique_ids when present
        #[tokio::test]
        async fn adds_faction_ids_to_unique_ids_when_present() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (corporation_id, mock_corporation) =
                test.eve()
                    .with_mock_corporation(98000001, None, Some(500001));
            let _corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: vec![corporation_id].into_iter().collect(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_missing_corporations(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            assert!(unique_ids.faction_ids.contains(&500001));

            Ok(())
        }
    }

    mod fetch_missing_alliances {
        use super::*;

        /// Expect Ok with fetched alliances when alliances are missing from database
        #[tokio::test]
        async fn fetches_missing_alliances_from_esi() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(99000001, None);
            let alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: vec![alliance_id].into_iter().collect(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_missing_alliances(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            let fetched = result.unwrap();
            assert_eq!(fetched.len(), 1);
            assert_eq!(fetched[0].0, alliance_id);

            alliance_endpoint.assert();

            Ok(())
        }

        /// Expect Ok with empty vec when no alliances are missing
        #[tokio::test]
        async fn returns_empty_when_no_alliances_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let alliance = test.eve().insert_mock_alliance(99000001, None).await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: vec![(alliance.alliance_id, alliance.id)]
                    .into_iter()
                    .collect(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: vec![alliance.alliance_id].into_iter().collect(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_missing_alliances(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            let fetched = result.unwrap();
            assert_eq!(fetched.len(), 0);

            Ok(())
        }

        /// Expect Ok with empty vec when input is empty
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_missing_alliances(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            let fetched = result.unwrap();
            assert_eq!(fetched.len(), 0);

            Ok(())
        }

        /// Expect Ok and verify faction_ids are added to unique_ids when present
        #[tokio::test]
        async fn adds_faction_ids_to_unique_ids_when_present() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (alliance_id, mock_alliance) =
                test.eve().with_mock_alliance(99000001, Some(500001));
            let _alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: vec![alliance_id].into_iter().collect(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_missing_alliances(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            assert!(unique_ids.faction_ids.contains(&500001));

            Ok(())
        }
    }

    mod attempt_update_missing_factions {
        use super::*;

        /// Expect Ok when no factions are missing
        #[tokio::test]
        async fn returns_ok_when_no_factions_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let faction = test.eve().insert_mock_faction(500001).await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: vec![(faction.faction_id, faction.id)].into_iter().collect(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: vec![faction.faction_id].into_iter().collect(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service
                .attempt_update_missing_factions(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok when input is empty
        #[tokio::test]
        async fn returns_ok_for_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service
                .attempt_update_missing_factions(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok and verify table_ids are updated when factions are fetched
        #[tokio::test]
        async fn updates_table_ids_when_factions_fetched() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let mock_faction = test.eve().with_mock_faction(500001);
            let _faction_endpoint = test.eve().with_faction_endpoint(vec![mock_faction], 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: vec![500001].into_iter().collect(),
                alliance_ids: HashSet::new(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service
                .attempt_update_missing_factions(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            assert_eq!(table_ids.faction_ids.len(), 1);
            assert!(table_ids.faction_ids.contains_key(&500001));

            Ok(())
        }
    }

    mod store_fetched_characters {
        use super::*;

        /// Expect Ok when storing fetched characters with valid corporation references
        #[tokio::test]
        async fn stores_characters_with_valid_corporation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let (character_id, character) = test
                .eve()
                .with_mock_character(2114794365, 98000001, None, None);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            let fetched_characters = vec![(character_id, character)];

            let result = service
                .store_fetched_characters(fetched_characters, &table_ids)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok but skip characters when corporation reference is missing
        #[tokio::test]
        async fn skips_characters_with_missing_corporation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (character_id, character) = test
                .eve()
                .with_mock_character(2114794365, 98000001, None, None);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(), // No corporation mapping
                character_ids: HashMap::new(),
            };

            let fetched_characters = vec![(character_id, character)];

            let result = service
                .store_fetched_characters(fetched_characters, &table_ids)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok when storing characters with faction references
        #[tokio::test]
        async fn stores_characters_with_faction_reference() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let faction = test.eve().insert_mock_faction(500001).await?;
            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let (character_id, character) =
                test.eve()
                    .with_mock_character(2114794365, 98000001, None, Some(500001));

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: vec![(500001, faction.id)].into_iter().collect(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            let fetched_characters = vec![(character_id, character)];

            let result = service
                .store_fetched_characters(fetched_characters, &table_ids)
                .await;

            assert!(result.is_ok());

            Ok(())
        }
    }

    mod store_fetched_corporations {
        use super::*;

        /// Expect Ok when storing fetched corporations
        #[tokio::test]
        async fn stores_corporations_and_updates_table_ids() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (corporation_id, corporation) =
                test.eve().with_mock_corporation(98000001, None, None);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let fetched_corporations = vec![(corporation_id, corporation)];

            let result = service
                .store_fetched_corporations(fetched_corporations, &mut table_ids)
                .await;

            assert!(result.is_ok());
            assert_eq!(table_ids.corporation_ids.len(), 1);
            assert!(table_ids.corporation_ids.contains_key(&corporation_id));

            Ok(())
        }

        /// Expect Ok when storing corporations with alliance references
        #[tokio::test]
        async fn stores_corporations_with_alliance_reference() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let alliance = test.eve().insert_mock_alliance(99000001, None).await?;

            let (corporation_id, corporation) =
                test.eve()
                    .with_mock_corporation(98000001, Some(99000001), None);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let fetched_corporations = vec![(corporation_id, corporation)];

            let result = service
                .store_fetched_corporations(fetched_corporations, &mut table_ids)
                .await;

            assert!(result.is_ok());
            assert!(table_ids.corporation_ids.contains_key(&corporation_id));

            Ok(())
        }

        /// Expect Ok when storing corporations with faction references
        #[tokio::test]
        async fn stores_corporations_with_faction_reference() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let faction = test.eve().insert_mock_faction(500001).await?;

            let (corporation_id, corporation) =
                test.eve()
                    .with_mock_corporation(98000001, None, Some(500001));

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: vec![(500001, faction.id)].into_iter().collect(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let fetched_corporations = vec![(corporation_id, corporation)];

            let result = service
                .store_fetched_corporations(fetched_corporations, &mut table_ids)
                .await;

            assert!(result.is_ok());
            assert!(table_ids.corporation_ids.contains_key(&corporation_id));

            Ok(())
        }
    }

    mod store_fetched_alliances {
        use super::*;

        /// Expect Ok when storing fetched alliances
        #[tokio::test]
        async fn stores_alliances_and_updates_table_ids() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (alliance_id, alliance) = test.eve().with_mock_alliance(99000001, None);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let fetched_alliances = vec![(alliance_id, alliance)];

            let result = service
                .store_fetched_alliances(fetched_alliances, &mut table_ids)
                .await;

            assert!(result.is_ok());
            assert_eq!(table_ids.alliance_ids.len(), 1);
            assert!(table_ids.alliance_ids.contains_key(&alliance_id));

            Ok(())
        }

        /// Expect Ok when storing alliances with faction references
        #[tokio::test]
        async fn stores_alliances_with_faction_reference() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let faction = test.eve().insert_mock_faction(500001).await?;

            let (alliance_id, alliance) = test.eve().with_mock_alliance(99000001, Some(500001));

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: vec![(500001, faction.id)].into_iter().collect(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let fetched_alliances = vec![(alliance_id, alliance)];

            let result = service
                .store_fetched_alliances(fetched_alliances, &mut table_ids)
                .await;

            assert!(result.is_ok());
            assert!(table_ids.alliance_ids.contains_key(&alliance_id));

            Ok(())
        }
    }

    mod fetch_and_store_missing_entities {
        use super::*;

        /// Expect Ok when all entities are already present
        #[tokio::test]
        async fn returns_ok_when_no_entities_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let faction = test.eve().insert_mock_faction(500001).await?;
            let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let character = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: vec![(faction.faction_id, faction.id)].into_iter().collect(),
                alliance_ids: vec![(alliance.alliance_id, alliance.id)]
                    .into_iter()
                    .collect(),
                corporation_ids: vec![(corporation.corporation_id, corporation.id)]
                    .into_iter()
                    .collect(),
                character_ids: vec![(character.character_id, character.id)]
                    .into_iter()
                    .collect(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: vec![faction.faction_id].into_iter().collect(),
                alliance_ids: vec![alliance.alliance_id].into_iter().collect(),
                corporation_ids: vec![corporation.corporation_id].into_iter().collect(),
                character_ids: vec![character.character_id].into_iter().collect(),
            };

            let result = service
                .fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok when fetching and storing missing characters
        #[tokio::test]
        async fn fetches_and_stores_missing_characters() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let (character_id, mock_character) = test
                .eve()
                .with_mock_character(2114794365, 98000001, None, None);
            let _character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: vec![98000001].into_iter().collect(),
                character_ids: vec![character_id].into_iter().collect(),
            };

            let result = service
                .fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok when fetching and storing missing corporations
        #[tokio::test]
        async fn fetches_and_stores_missing_corporations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (corporation_id, mock_corporation) =
                test.eve().with_mock_corporation(98000001, None, None);
            let _corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: HashSet::new(),
                corporation_ids: vec![corporation_id].into_iter().collect(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            assert!(table_ids.corporation_ids.contains_key(&corporation_id));

            Ok(())
        }

        /// Expect Ok when fetching and storing missing alliances
        #[tokio::test]
        async fn fetches_and_stores_missing_alliances() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(99000001, None);
            let _alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: vec![alliance_id].into_iter().collect(),
                corporation_ids: HashSet::new(),
                character_ids: HashSet::new(),
            };

            let result = service
                .fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            assert!(table_ids.alliance_ids.contains_key(&alliance_id));

            Ok(())
        }

        /// Expect Ok when fetching entities with pre-populated dependencies
        #[tokio::test]
        async fn fetches_entities_with_dependencies() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(99000001, None);
            let _alliance_endpoint =
                test.eve()
                    .with_alliance_endpoint(alliance_id, mock_alliance, 1);

            let (corporation_id, mock_corporation) =
                test.eve()
                    .with_mock_corporation(98000001, Some(99000001), None);
            let _corporation_endpoint =
                test.eve()
                    .with_corporation_endpoint(corporation_id, mock_corporation, 1);

            let (character_id, mock_character) = test
                .eve()
                .with_mock_character(2114794365, 98000001, None, None);
            let _character_endpoint =
                test.eve()
                    .with_character_endpoint(character_id, mock_character, 1);

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let mut table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            // Pre-populate unique_ids with all IDs to simulate a proper affiliation update
            let mut unique_ids = UniqueIds {
                faction_ids: HashSet::new(),
                alliance_ids: vec![alliance_id].into_iter().collect(),
                corporation_ids: vec![corporation_id].into_iter().collect(),
                character_ids: vec![character_id].into_iter().collect(),
            };

            let result = service
                .fetch_and_store_missing_entities(&mut table_ids, &mut unique_ids)
                .await;

            assert!(result.is_ok());
            // Verify that entities were stored and added to table_ids
            assert!(table_ids.alliance_ids.contains_key(&alliance_id));
            assert!(table_ids.corporation_ids.contains_key(&corporation_id));

            Ok(())
        }
    }

    mod update_character_affiliations {
        use super::*;
        use eve_esi::model::character::CharacterAffiliation;

        /// Expect Ok when updating character affiliations with valid references
        #[tokio::test]
        async fn updates_character_affiliations_successfully() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let character = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: vec![(2114794365, character.id)].into_iter().collect(),
            };

            let affiliations = vec![CharacterAffiliation {
                character_id: 2114794365,
                corporation_id: 98000001,
                alliance_id: None,
                faction_id: None,
            }];

            let result = service
                .update_character_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify the database was updated
            let updated_character = CharacterRepository::new(&test.state.db)
                .get_by_character_id(2114794365)
                .await?;
            assert!(updated_character.is_some());
            let updated_character = updated_character.unwrap();
            assert_eq!(updated_character.corporation_id, corporation.id);
            assert_eq!(updated_character.faction_id, None);

            Ok(())
        }

        /// Expect Ok when updating character affiliations with faction references
        #[tokio::test]
        async fn updates_character_affiliations_with_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let faction = test.eve().insert_mock_faction(500001).await?;
            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let character = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: vec![(500001, faction.id)].into_iter().collect(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: vec![(2114794365, character.id)].into_iter().collect(),
            };

            let affiliations = vec![CharacterAffiliation {
                character_id: 2114794365,
                corporation_id: 98000001,
                alliance_id: None,
                faction_id: Some(500001),
            }];

            let result = service
                .update_character_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify the database was updated with faction
            let updated_character = CharacterRepository::new(&test.state.db)
                .get_by_character_id(2114794365)
                .await?;
            assert!(updated_character.is_some());
            let updated_character = updated_character.unwrap();
            assert_eq!(updated_character.corporation_id, corporation.id);
            assert_eq!(updated_character.faction_id, Some(faction.id));

            Ok(())
        }

        /// Expect Ok but skip affiliations when character is not found
        #[tokio::test]
        async fn skips_affiliations_when_character_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(), // Character not in table_ids
            };

            let affiliations = vec![CharacterAffiliation {
                character_id: 2114794365,
                corporation_id: 98000001,
                alliance_id: None,
                faction_id: None,
            }];

            let result = service
                .update_character_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify no character was created/updated
            let character = CharacterRepository::new(&test.state.db)
                .get_by_character_id(2114794365)
                .await?;
            assert!(character.is_none());

            Ok(())
        }

        /// Expect Ok but skip affiliations when corporation is not found
        #[tokio::test]
        async fn skips_affiliations_when_corporation_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let character = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;

            let original_corporation_id = character.corporation_id;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(), // Corporation not in table_ids
                character_ids: vec![(2114794365, character.id)].into_iter().collect(),
            };

            let affiliations = vec![CharacterAffiliation {
                character_id: 2114794365,
                corporation_id: 98000001,
                alliance_id: None,
                faction_id: None,
            }];

            let result = service
                .update_character_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify character was not updated (corporation_id should remain unchanged)
            let character_after = CharacterRepository::new(&test.state.db)
                .get_by_character_id(2114794365)
                .await?;
            assert!(character_after.is_some());
            assert_eq!(
                character_after.unwrap().corporation_id,
                original_corporation_id
            );

            Ok(())
        }

        /// Expect Ok and set faction to None when faction is not found
        #[tokio::test]
        async fn sets_faction_to_none_when_faction_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let character = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(), // Faction not in table_ids
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: vec![(2114794365, character.id)].into_iter().collect(),
            };

            let affiliations = vec![CharacterAffiliation {
                character_id: 2114794365,
                corporation_id: 98000001,
                alliance_id: None,
                faction_id: Some(500001), // Faction not found, should be set to None
            }];

            let result = service
                .update_character_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify faction was set to None
            let updated_character = CharacterRepository::new(&test.state.db)
                .get_by_character_id(2114794365)
                .await?;
            assert!(updated_character.is_some());
            let updated_character = updated_character.unwrap();
            assert_eq!(updated_character.faction_id, None);

            Ok(())
        }

        /// Expect Ok when updating multiple character affiliations
        #[tokio::test]
        async fn updates_multiple_character_affiliations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation1 = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let corporation2 = test
                .eve()
                .insert_mock_corporation(98000002, None, None)
                .await?;
            let character1 = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;
            let character2 = test
                .eve()
                .insert_mock_character(2114794366, 98000002, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation1.id), (98000002, corporation2.id)]
                    .into_iter()
                    .collect(),
                character_ids: vec![(2114794365, character1.id), (2114794366, character2.id)]
                    .into_iter()
                    .collect(),
            };

            let affiliations = vec![
                CharacterAffiliation {
                    character_id: 2114794365,
                    corporation_id: 98000001,
                    alliance_id: None,
                    faction_id: None,
                },
                CharacterAffiliation {
                    character_id: 2114794366,
                    corporation_id: 98000002,
                    alliance_id: None,
                    faction_id: None,
                },
            ];

            let result = service
                .update_character_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify both characters were updated
            let updated_char1 = CharacterRepository::new(&test.state.db)
                .get_by_character_id(2114794365)
                .await?;
            assert!(updated_char1.is_some());
            assert_eq!(updated_char1.unwrap().corporation_id, corporation1.id);

            let updated_char2 = CharacterRepository::new(&test.state.db)
                .get_by_character_id(2114794366)
                .await?;
            assert!(updated_char2.is_some());
            assert_eq!(updated_char2.unwrap().corporation_id, corporation2.id);

            Ok(())
        }

        /// Expect Ok when deduplicating character affiliations
        #[tokio::test]
        async fn deduplicates_character_affiliations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let character = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: vec![(2114794365, character.id)].into_iter().collect(),
            };

            // Duplicate affiliations
            let affiliations = vec![
                CharacterAffiliation {
                    character_id: 2114794365,
                    corporation_id: 98000001,
                    alliance_id: None,
                    faction_id: None,
                },
                CharacterAffiliation {
                    character_id: 2114794365,
                    corporation_id: 98000001,
                    alliance_id: None,
                    faction_id: None,
                },
            ];

            let result = service
                .update_character_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify character was updated (deduplication should handle duplicates)
            let updated_character = CharacterRepository::new(&test.state.db)
                .get_by_character_id(2114794365)
                .await?;
            assert!(updated_character.is_some());
            let updated_character = updated_character.unwrap();
            assert_eq!(updated_character.corporation_id, corporation.id);

            Ok(())
        }

        /// Expect Ok when processing empty affiliations list
        #[tokio::test]
        async fn handles_empty_affiliations_list() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let affiliations: Vec<CharacterAffiliation> = vec![];

            let result = service
                .update_character_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify no updates occurred (should be no-op for empty list)
            // This test just ensures the method handles empty input gracefully
            // No database state to verify since no entities were involved

            Ok(())
        }

        /// Expect Ok when processing mixed valid and invalid affiliations
        #[tokio::test]
        async fn processes_mixed_valid_and_invalid_affiliations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let character1 = test
                .eve()
                .insert_mock_character(2114794365, 98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: vec![(2114794365, character1.id)].into_iter().collect(),
            };

            let affiliations = vec![
                // Valid affiliation
                CharacterAffiliation {
                    character_id: 2114794365,
                    corporation_id: 98000001,
                    alliance_id: None,
                    faction_id: None,
                },
                // Invalid - character not found
                CharacterAffiliation {
                    character_id: 9999999999,
                    corporation_id: 98000001,
                    alliance_id: None,
                    faction_id: None,
                },
                // Invalid - corporation not found
                CharacterAffiliation {
                    character_id: 2114794365,
                    corporation_id: 9999999999,
                    alliance_id: None,
                    faction_id: None,
                },
            ];

            let result = service
                .update_character_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify valid affiliation was processed
            let updated_char = CharacterRepository::new(&test.state.db)
                .get_by_character_id(2114794365)
                .await?;
            assert!(updated_char.is_some());
            assert_eq!(updated_char.unwrap().corporation_id, corporation.id);

            // Verify invalid character was not created
            let invalid_char = CharacterRepository::new(&test.state.db)
                .get_by_character_id(9999999999)
                .await?;
            assert!(invalid_char.is_none());

            Ok(())
        }
    }

    mod update_corporation_affiliations {
        use super::*;
        use eve_esi::model::character::CharacterAffiliation;

        /// Expect Ok when updating corporation affiliations with alliance
        #[tokio::test]
        async fn updates_corporation_affiliations_with_alliance() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;

            let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            let affiliations = vec![CharacterAffiliation {
                character_id: 2114794365, // Character ID doesn't matter for corporation updates
                corporation_id: 98000001,
                alliance_id: Some(99000001),
                faction_id: None,
            }];

            let result = service
                .update_corporation_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify the database was updated
            let updated_corporation = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(98000001)
                .await?;
            assert!(updated_corporation.is_some());
            let updated_corporation = updated_corporation.unwrap();
            assert_eq!(updated_corporation.alliance_id, Some(alliance.id));

            Ok(())
        }

        /// Expect Ok when updating corporation affiliations without alliance
        #[tokio::test]
        async fn updates_corporation_affiliations_without_alliance() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;

            let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, Some(99000001), None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            let affiliations = vec![CharacterAffiliation {
                character_id: 2114794365,
                corporation_id: 98000001,
                alliance_id: None, // Removing alliance affiliation
                faction_id: None,
            }];

            let result = service
                .update_corporation_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify the alliance was removed
            let updated_corporation = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(98000001)
                .await?;
            assert!(updated_corporation.is_some());
            let updated_corporation = updated_corporation.unwrap();
            assert_eq!(updated_corporation.alliance_id, None);

            Ok(())
        }

        /// Expect Ok but skip affiliations when corporation is not found
        #[tokio::test]
        async fn skips_affiliations_when_corporation_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;

            let alliance = test.eve().insert_mock_alliance(99000001, None).await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
                corporation_ids: HashMap::new(), // Corporation not in table_ids
                character_ids: HashMap::new(),
            };

            let affiliations = vec![CharacterAffiliation {
                character_id: 2114794365,
                corporation_id: 98000001,
                alliance_id: Some(99000001),
                faction_id: None,
            }];

            let result = service
                .update_corporation_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify no corporation was created/updated
            let corporation = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(98000001)
                .await?;
            assert!(corporation.is_none());

            Ok(())
        }

        /// Expect Ok but skip affiliations when alliance is not found
        #[tokio::test]
        async fn skips_affiliations_when_alliance_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;

            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let original_alliance_id = corporation.alliance_id;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(), // Alliance not in table_ids
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            let affiliations = vec![CharacterAffiliation {
                character_id: 2114794365,
                corporation_id: 98000001,
                alliance_id: Some(99000001), // Alliance not found
                faction_id: None,
            }];

            let result = service
                .update_corporation_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify corporation was not updated (alliance_id should remain unchanged)
            let corporation_after = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(98000001)
                .await?;
            assert!(corporation_after.is_some());
            assert_eq!(corporation_after.unwrap().alliance_id, original_alliance_id);

            Ok(())
        }

        /// Expect Ok when updating multiple corporation affiliations
        #[tokio::test]
        async fn updates_multiple_corporation_affiliations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;

            let alliance1 = test.eve().insert_mock_alliance(99000001, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(99000002, None).await?;
            let corporation1 = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let corporation2 = test
                .eve()
                .insert_mock_corporation(98000002, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: vec![(99000001, alliance1.id), (99000002, alliance2.id)]
                    .into_iter()
                    .collect(),
                corporation_ids: vec![(98000001, corporation1.id), (98000002, corporation2.id)]
                    .into_iter()
                    .collect(),
                character_ids: HashMap::new(),
            };

            let affiliations = vec![
                CharacterAffiliation {
                    character_id: 2114794365,
                    corporation_id: 98000001,
                    alliance_id: Some(99000001),
                    faction_id: None,
                },
                CharacterAffiliation {
                    character_id: 2114794366,
                    corporation_id: 98000002,
                    alliance_id: Some(99000002),
                    faction_id: None,
                },
            ];

            let result = service
                .update_corporation_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify both corporations were updated
            let updated_corp1 = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(98000001)
                .await?;
            assert!(updated_corp1.is_some());
            assert_eq!(updated_corp1.unwrap().alliance_id, Some(alliance1.id));

            let updated_corp2 = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(98000002)
                .await?;
            assert!(updated_corp2.is_some());
            assert_eq!(updated_corp2.unwrap().alliance_id, Some(alliance2.id));

            Ok(())
        }

        /// Expect Ok when deduplicating corporation affiliations
        #[tokio::test]
        async fn deduplicates_corporation_affiliations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;

            let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
            let corporation = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
                corporation_ids: vec![(98000001, corporation.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            // Duplicate affiliations (from different characters in same corporation)
            let affiliations = vec![
                CharacterAffiliation {
                    character_id: 2114794365,
                    corporation_id: 98000001,
                    alliance_id: Some(99000001),
                    faction_id: None,
                },
                CharacterAffiliation {
                    character_id: 2114794366, // Different character, same corporation
                    corporation_id: 98000001,
                    alliance_id: Some(99000001),
                    faction_id: None,
                },
            ];

            let result = service
                .update_corporation_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify corporation was updated (deduplication should handle duplicates)
            let updated_corporation = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(98000001)
                .await?;
            assert!(updated_corporation.is_some());
            let updated_corporation = updated_corporation.unwrap();
            assert_eq!(updated_corporation.alliance_id, Some(alliance.id));

            Ok(())
        }

        /// Expect Ok when processing empty affiliations list
        #[tokio::test]
        async fn handles_empty_affiliations_list() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: HashMap::new(),
                corporation_ids: HashMap::new(),
                character_ids: HashMap::new(),
            };

            let affiliations: Vec<CharacterAffiliation> = vec![];

            let result = service
                .update_corporation_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify no updates occurred (should be no-op for empty list)
            // This test just ensures the method handles empty input gracefully
            // No database state to verify since no entities were involved

            Ok(())
        }

        /// Expect Ok when processing mixed valid and invalid affiliations
        #[tokio::test]
        async fn processes_mixed_valid_and_invalid_affiliations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;

            let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
            let corporation1 = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
                corporation_ids: vec![(98000001, corporation1.id)].into_iter().collect(),
                character_ids: HashMap::new(),
            };

            let affiliations = vec![
                // Valid affiliation
                CharacterAffiliation {
                    character_id: 2114794365,
                    corporation_id: 98000001,
                    alliance_id: Some(99000001),
                    faction_id: None,
                },
                // Invalid - corporation not found
                CharacterAffiliation {
                    character_id: 2114794366,
                    corporation_id: 9999999999,
                    alliance_id: Some(99000001),
                    faction_id: None,
                },
                // Invalid - alliance not found
                CharacterAffiliation {
                    character_id: 2114794367,
                    corporation_id: 98000001,
                    alliance_id: Some(9999999999),
                    faction_id: None,
                },
            ];

            let result = service
                .update_corporation_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify valid affiliation was processed
            let updated_corp = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(98000001)
                .await?;
            assert!(updated_corp.is_some());
            assert_eq!(updated_corp.unwrap().alliance_id, Some(alliance.id));

            // Verify invalid corporation was not created
            let invalid_corp = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(9999999999)
                .await?;
            assert!(invalid_corp.is_none());

            Ok(())
        }

        /// Expect Ok when updating corporation with mixed alliance statuses
        #[tokio::test]
        async fn updates_corporation_with_mixed_alliance_statuses() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
            )?;

            let alliance = test.eve().insert_mock_alliance(99000001, None).await?;
            let corporation1 = test
                .eve()
                .insert_mock_corporation(98000001, None, None)
                .await?;
            let corporation2 = test
                .eve()
                .insert_mock_corporation(98000002, Some(99000001), None)
                .await?;

            let service = AffiliationService {
                db: &test.state.db,
                esi_client: &test.state.esi_client,
            };

            let table_ids = TableIds {
                faction_ids: HashMap::new(),
                alliance_ids: vec![(99000001, alliance.id)].into_iter().collect(),
                corporation_ids: vec![(98000001, corporation1.id), (98000002, corporation2.id)]
                    .into_iter()
                    .collect(),
                character_ids: HashMap::new(),
            };

            let affiliations = vec![
                // Add alliance to corporation1
                CharacterAffiliation {
                    character_id: 2114794365,
                    corporation_id: 98000001,
                    alliance_id: Some(99000001),
                    faction_id: None,
                },
                // Remove alliance from corporation2
                CharacterAffiliation {
                    character_id: 2114794366,
                    corporation_id: 98000002,
                    alliance_id: None,
                    faction_id: None,
                },
            ];

            let result = service
                .update_corporation_affiliations(&affiliations, &table_ids)
                .await;

            assert!(result.is_ok());

            // Verify corporation1 now has alliance
            let updated_corp1 = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(98000001)
                .await?;
            assert!(updated_corp1.is_some());
            assert_eq!(updated_corp1.unwrap().alliance_id, Some(alliance.id));

            // Verify corporation2 no longer has alliance
            let updated_corp2 = CorporationRepository::new(&test.state.db)
                .get_by_corporation_id(98000002)
                .await?;
            assert!(updated_corp2.is_some());
            assert_eq!(updated_corp2.unwrap().alliance_id, None);

            Ok(())
        }
    }
}
