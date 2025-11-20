use crate::server::service::eve::affiliation::AffiliationService;

use super::*;

impl AffiliationService {
    // Updates a corporation's affiliated alliance
    pub(super) async fn update_corporation_affiliations(
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
    pub(super) async fn update_character_affiliations(
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
}
