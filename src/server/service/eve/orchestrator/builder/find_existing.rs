use std::collections::{HashMap, HashSet};

use super::EveEntityOrchestratorBuilder;
use crate::server::{
    data::eve::{
        alliance::AllianceRepository, character::CharacterRepository,
        corporation::CorporationRepository, faction::FactionRepository,
    },
    error::AppError,
};

impl<'a> EveEntityOrchestratorBuilder<'a> {
    /// Finds characters related to requested entities within the database.
    ///
    /// Queries the database for dependency characters to avoid redundant ESI calls.
    /// Returns both found characters and IDs that need to be fetched.
    ///
    /// # Arguments
    /// - `dependency_character_ids` - IDs of characters to check in the database
    ///
    /// # Returns
    /// - `Ok((HashMap<i64, i32>, Vec<i64>))` - Tuple of:
    ///   - Map of EVE character IDs to their database record IDs
    ///   - Vector of EVE character IDs not found in the database
    /// - `Err(AppError::DbErr)` - Database query failed
    pub(super) async fn find_existing_characters(
        &self,
        dependency_character_ids: &[i64],
    ) -> Result<(HashMap<i64, i32>, Vec<i64>), AppError> {
        let character_repo = CharacterRepository::new(self.db);

        let character_record_ids = character_repo
            .get_record_ids_by_character_ids(dependency_character_ids)
            .await?;

        let existing_character_ids: HashSet<i64> = character_record_ids
            .iter()
            .map(|(_, character_id)| *character_id)
            .collect();

        let mut missing_character_ids = Vec::new();

        for &dep_char_id in dependency_character_ids {
            if !existing_character_ids.contains(&dep_char_id) {
                missing_character_ids.push(dep_char_id);
            }
        }

        Ok((
            character_record_ids
                .into_iter()
                .map(|(record_id, character_id)| (character_id, record_id))
                .collect(),
            missing_character_ids,
        ))
    }

    /// Finds corporations related to requested entities within the database.
    ///
    /// Queries the database for dependency corporations to avoid redundant ESI calls.
    /// Returns both found corporations and IDs that need to be fetched.
    ///
    /// # Arguments
    /// - `dependency_corporation_ids` - IDs of corporations to check in the database
    ///
    /// # Returns
    /// - `Ok((HashMap<i64, i32>, Vec<i64>))` - Tuple of:
    ///   - Map of EVE corporation IDs to their database record IDs
    ///   - Vector of EVE corporation IDs not found in the database
    /// - `Err(AppError::DbErr)` - Database query failed
    pub(super) async fn find_existing_corporations(
        &self,
        dependency_corporation_ids: &[i64],
    ) -> Result<(HashMap<i64, i32>, Vec<i64>), AppError> {
        let corporation_repo = CorporationRepository::new(self.db);

        let corporation_record_ids = corporation_repo
            .get_record_ids_by_corporation_ids(dependency_corporation_ids)
            .await?;

        let existing_corporation_ids: HashSet<i64> = corporation_record_ids
            .iter()
            .map(|(_, corp_id)| *corp_id)
            .collect();

        let mut missing_corporation_ids = Vec::new();

        for &dep_corp_id in dependency_corporation_ids {
            if !existing_corporation_ids.contains(&dep_corp_id) {
                missing_corporation_ids.push(dep_corp_id);
            }
        }

        Ok((
            corporation_record_ids
                .into_iter()
                .map(|(record_id, corp_id)| (corp_id, record_id))
                .collect(),
            missing_corporation_ids,
        ))
    }

    /// Finds alliances related to requested entities within the database.
    ///
    /// Queries the database for dependency alliances to avoid redundant ESI calls.
    /// Returns both found alliances and IDs that need to be fetched.
    ///
    /// # Arguments
    /// - `dependency_alliance_ids` - IDs of alliances to check in the database
    ///
    /// # Returns
    /// - `Ok((HashMap<i64, i32>, Vec<i64>))` - Tuple of:
    ///   - Map of EVE alliance IDs to their database record IDs
    ///   - Vector of EVE alliance IDs not found in the database
    /// - `Err(AppError::DbErr)` - Database query failed
    pub(super) async fn find_existing_alliances(
        &self,
        dependency_alliance_ids: &[i64],
    ) -> Result<(HashMap<i64, i32>, Vec<i64>), AppError> {
        let alliance_repo = AllianceRepository::new(self.db);

        let alliance_record_ids = alliance_repo
            .get_record_ids_by_alliance_ids(dependency_alliance_ids)
            .await?;

        let existing_alliance_ids: HashSet<i64> = alliance_record_ids
            .iter()
            .map(|(_, alliance_id)| *alliance_id)
            .collect();

        let mut missing_alliance_ids = Vec::new();

        for &dep_alliance_id in dependency_alliance_ids {
            if !existing_alliance_ids.contains(&dep_alliance_id) {
                missing_alliance_ids.push(dep_alliance_id);
            }
        }

        Ok((
            alliance_record_ids
                .into_iter()
                .map(|(record_id, alliance_id)| (alliance_id, record_id))
                .collect(),
            missing_alliance_ids,
        ))
    }

    /// Finds factions related to requested entities within the database.
    ///
    /// Queries the database for dependency factions to avoid redundant ESI calls.
    /// Returns both found factions and IDs that need to be fetched.
    ///
    /// # Arguments
    /// - `dependency_faction_ids` - IDs of factions to check in the database
    ///
    /// # Returns
    /// - `Ok((HashMap<i64, i32>, Vec<i64>))` - Tuple of:
    ///   - Map of EVE faction IDs to their database record IDs
    ///   - Vector of EVE faction IDs not found in the database
    /// - `Err(AppError::DbErr)` - Database query failed
    pub(super) async fn find_existing_factions(
        &self,
        dependency_faction_ids: &[i64],
    ) -> Result<(HashMap<i64, i32>, Vec<i64>), AppError> {
        let faction_repo = FactionRepository::new(self.db);

        let faction_record_ids = faction_repo
            .get_record_ids_by_faction_ids(dependency_faction_ids)
            .await?;

        let existing_faction_ids: HashSet<i64> = faction_record_ids
            .iter()
            .map(|(_, faction_id)| *faction_id)
            .collect();

        let mut missing_faction_ids = Vec::new();

        for &dep_faction_id in dependency_faction_ids {
            if !existing_faction_ids.contains(&dep_faction_id) {
                missing_faction_ids.push(dep_faction_id);
            }
        }

        Ok((
            faction_record_ids
                .into_iter()
                .map(|(record_id, faction_id)| (faction_id, record_id))
                .collect(),
            missing_faction_ids,
        ))
    }
}
