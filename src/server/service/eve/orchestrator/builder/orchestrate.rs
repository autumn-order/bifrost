use std::collections::HashMap;

use super::{EveEntityOrchestratorBuilder, FactionFetchState};
use crate::server::error::Error;

impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Orchestrates the complete character workflow: find existing, fetch missing, extract dependencies.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, i32>)` - Map of character IDs to database record IDs
    /// - `Err(Error)` - Database or ESI error occurred
    pub(super) async fn orchestrate_characters(&mut self) -> Result<HashMap<i64, i32>, Error> {
        let dependency_character_ids: Vec<i64> =
            self.dependency_character_ids.iter().copied().collect();
        let (characters_record_id_map, missing_character_ids) = self
            .find_existing_characters(&dependency_character_ids)
            .await?;
        self.requested_character_ids.extend(missing_character_ids);

        let character_ids: Vec<i64> = self.requested_character_ids.iter().copied().collect();
        let fetched_characters = self.fetch_characters(character_ids).await?;

        self.characters_map.extend(fetched_characters);

        // Extract dependencies from all characters in the map
        for character in self.characters_map.values() {
            self.dependency_corporation_ids
                .insert(character.corporation_id);

            if let Some(faction_id) = character.faction_id {
                self.dependency_faction_ids.insert(faction_id);
            }
        }

        Ok(characters_record_id_map)
    }

    /// Orchestrates the complete corporation workflow: find existing, fetch missing, extract dependencies.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, i32>)` - Map of corporation IDs to database record IDs
    /// - `Err(Error)` - Database or ESI error occurred
    pub(super) async fn orchestrate_corporations(&mut self) -> Result<HashMap<i64, i32>, Error> {
        let dependency_corporation_ids: Vec<i64> =
            self.dependency_corporation_ids.iter().copied().collect();
        let (corporations_record_id_map, missing_corporation_ids) = self
            .find_existing_corporations(&dependency_corporation_ids)
            .await?;
        self.requested_corporation_ids
            .extend(missing_corporation_ids);

        let corporation_ids: Vec<i64> = self.requested_corporation_ids.iter().copied().collect();
        let fetched_corporations = self.fetch_corporations(corporation_ids).await?;

        self.corporations_map.extend(fetched_corporations);

        // Extract dependencies from all corporations in the map
        for corporation in self.corporations_map.values() {
            if let Some(alliance_id) = corporation.alliance_id {
                self.dependency_alliance_ids.insert(alliance_id);
            }

            if let Some(faction_id) = corporation.faction_id {
                self.dependency_faction_ids.insert(faction_id);
            }
        }

        Ok(corporations_record_id_map)
    }

    /// Orchestrates the complete alliance workflow: find existing, fetch missing, extract dependencies.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, i32>)` - Map of alliance IDs to database record IDs
    /// - `Err(Error)` - Database or ESI error occurred
    pub(super) async fn orchestrate_alliances(&mut self) -> Result<HashMap<i64, i32>, Error> {
        let dependency_alliance_ids: Vec<i64> =
            self.dependency_alliance_ids.iter().copied().collect();
        let (alliances_record_id_map, missing_alliance_ids) = self
            .find_existing_alliances(&dependency_alliance_ids)
            .await?;
        self.requested_alliance_ids.extend(missing_alliance_ids);

        let alliance_ids: Vec<i64> = self.requested_alliance_ids.iter().copied().collect();
        let fetched_alliances = self.fetch_alliances(alliance_ids).await?;

        self.alliances_map.extend(fetched_alliances);

        // Extract dependencies from all alliances in the map
        for alliance in self.alliances_map.values() {
            if let Some(faction_id) = alliance.faction_id {
                self.dependency_faction_ids.insert(faction_id);
            }
        }

        Ok(alliances_record_id_map)
    }

    /// Orchestrates the complete faction workflow: find existing, fetch if needed.
    ///
    /// Unlike other entities, factions are only fetched if:
    /// - Explicitly requested via `with_factions()` (periodic update)
    /// - Any entity references a faction we don't have (dependency resolution)
    ///
    /// # Returns
    /// - `Ok((HashMap<i64, i32>, FactionFetchState))` - Tuple of:
    ///   - Map of faction IDs to database record IDs
    ///   - Faction fetch state indicating what action was taken
    /// - `Err(Error)` - Database or ESI error occurred
    pub(super) async fn orchestrate_factions(
        &self,
    ) -> Result<(HashMap<i64, i32>, FactionFetchState), Error> {
        let dependency_faction_ids: Vec<i64> =
            self.dependency_faction_ids.iter().copied().collect();
        let (factions_record_id_map, missing_faction_ids) =
            self.find_existing_factions(&dependency_faction_ids).await?;

        let factions = if self.requested_faction_update || missing_faction_ids.len() > 0 {
            // Fetch factions if:
            // - Explicitly requested via with_factions() (periodic update)
            // - Any entity references a faction we don't have (dependency resolution)
            self.fetch_factions_if_stale().await?
        } else {
            // No factions requested
            FactionFetchState::NotRequested
        };

        Ok((factions_record_id_map, factions))
    }
}
