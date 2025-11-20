use crate::server::service::eve::affiliation::AffiliationService;

use super::*;

impl<'a> AffiliationService<'a> {
    pub(super) async fn attempt_update_missing_factions(
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

        let updated_factions = FactionService::new(self.db.clone(), self.esi_client.clone())
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

    pub(super) async fn store_fetched_characters(
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

    pub(super) async fn store_fetched_corporations(
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

    pub(super) async fn store_fetched_alliances(
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
