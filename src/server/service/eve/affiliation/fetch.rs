use crate::server::service::eve::affiliation::AffiliationService;

use super::*;

impl<'a> AffiliationService<'a> {
    pub(super) async fn fetch_missing_characters(
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

    pub(super) async fn fetch_missing_corporations(
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

        let fetched_corporations =
            CorporationService::new(self.db.clone(), self.esi_client.clone())
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

    pub(super) async fn fetch_missing_alliances(
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
}
