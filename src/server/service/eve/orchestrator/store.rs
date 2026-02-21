use std::collections::HashMap;

use dioxus_logger::tracing;
use entity::{
    eve_alliance::Model as EveAllianceModel, eve_character::Model as EveCharacterModel,
    eve_corporation::Model as EveCorporationModel, eve_faction::Model as EveFactionModel,
};
use eve_esi::model::{alliance::Alliance, character::Character, corporation::Corporation};
use sea_orm::DatabaseTransaction;

use super::{EveEntityOrchestrator, FactionFetchState};
use crate::server::{
    data::eve::{
        alliance::AllianceRepository, character::CharacterRepository,
        corporation::CorporationRepository, faction::FactionRepository,
    },
    error::AppError,
};

impl EveEntityOrchestrator {
    /// Handles faction storage based on fetch state.
    ///
    /// Encapsulates all faction storage logic, processing entities according to their fetch state:
    /// - `Fresh`: Upserts new faction data from ESI
    /// - `NotModified`: Updates timestamps for revalidated data (304 Not Modified)
    /// - `UpToDate`: Loads existing factions from database
    /// - `NotRequested`: Returns empty maps without database operations
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `factions_state` - The fetch state determining how to handle factions
    ///
    /// # Returns
    /// - `Ok((entity_map, record_id_map))` - Tuple containing:
    ///   - `HashMap<i64, EveFactionModel>` - Map of EVE faction IDs to stored database models
    ///   - `HashMap<i64, i32>` - Map of EVE faction IDs to database record IDs
    /// - `Err(AppError::Database)` - Database operation failed
    pub(super) async fn store_factions(
        &self,
        txn: &DatabaseTransaction,
        factions_state: FactionFetchState,
    ) -> Result<(HashMap<i64, EveFactionModel>, HashMap<i64, i32>), AppError> {
        let faction_repo = FactionRepository::new(txn);

        let stored_factions = match factions_state {
            FactionFetchState::Fresh(factions_map) => {
                // Store & return updated faction information fetched from ESI
                faction_repo
                    .upsert_many(
                        factions_map
                            .into_iter()
                            .map(|(_, faction)| faction)
                            .collect(),
                    )
                    .await?
            }
            FactionFetchState::NotModified => {
                // Update timestamps for revalidated data (304 Not Modified)
                faction_repo.update_all_timestamps().await?;
                faction_repo.get_all().await?
            }
            FactionFetchState::UpToDate => {
                // Factions are up-to-date, load existing from DB
                faction_repo.get_all().await?
            }
            FactionFetchState::NotRequested => {
                // No factions needed at all
                return Ok((HashMap::new(), HashMap::new()));
            }
        };

        let record_id_map = stored_factions
            .iter()
            .map(|faction| (faction.faction_id, faction.id))
            .collect();

        let factions_map = stored_factions
            .into_iter()
            .map(|faction| (faction.faction_id, faction))
            .collect();

        Ok((factions_map, record_id_map))
    }

    /// Stores alliance entities to the database with faction relationships.
    ///
    /// Upserts all fetched alliances, linking them to their factions if present.
    /// Returns empty maps immediately if no alliances are provided.
    /// Logs warnings for alliances with faction IDs that couldn't be resolved.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `alliances_map` - Map of alliance IDs to alliance ESI data (may be empty)
    ///
    /// # Returns
    /// - `Ok((entity_map, record_id_map))` - Tuple containing:
    ///   - `HashMap<i64, EveAllianceModel>` - Map of EVE alliance IDs to stored database models
    ///   - `HashMap<i64, i32>` - Map of EVE alliance IDs to database record IDs
    /// - `Err(AppError::Database)` - Database operation failed
    pub(super) async fn store_alliances(
        &self,
        txn: &DatabaseTransaction,
        alliances_map: HashMap<i64, Alliance>,
    ) -> Result<(HashMap<i64, EveAllianceModel>, HashMap<i64, i32>), AppError> {
        if alliances_map.is_empty() {
            return Ok((HashMap::new(), HashMap::new()));
        }

        let alliance_repo = AllianceRepository::new(txn);

        let alliance_relations = alliances_map.into_iter().map(|(alliance_id, alliance)| {
            let faction_record_id = alliance.faction_id.and_then(|faction_id| {
                match self.factions_record_id_map.get(&faction_id) {
                    Some(id) => Some(*id),
                    None => {
                        tracing::warn!(
                            faction_id = %faction_id,
                            alliance_id = %alliance_id,
                            "Failed to find faction record ID in database; alliance will have no related faction"
                        );
                        None
                    }
                }
            });

            (alliance_id, alliance, faction_record_id)
        }).collect();

        let stored_alliances = alliance_repo.upsert_many(alliance_relations).await?;

        let record_id_map = stored_alliances
            .iter()
            .map(|a| (a.alliance_id, a.id))
            .collect();

        let alliances_map = stored_alliances
            .into_iter()
            .map(|a| (a.alliance_id, a))
            .collect();

        Ok((alliances_map, record_id_map))
    }

    /// Stores corporation entities to the database with alliance and faction relationships.
    ///
    /// Upserts all fetched corporations, linking them to their alliances and factions if present.
    /// Returns empty maps immediately if no corporations are provided.
    /// Logs warnings for corporations with alliance or faction IDs that couldn't be resolved.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `corporations_map` - Map of corporation IDs to corporation ESI data (may be empty)
    ///
    /// # Returns
    /// - `Ok((entity_map, record_id_map))` - Tuple containing:
    ///   - `HashMap<i64, EveCorporationModel>` - Map of EVE corporation IDs to stored database models
    ///   - `HashMap<i64, i32>` - Map of EVE corporation IDs to database record IDs
    /// - `Err(AppError::Database)` - Database operation failed
    pub(super) async fn store_corporations(
        &self,
        txn: &DatabaseTransaction,
        corporations_map: HashMap<i64, Corporation>,
    ) -> Result<(HashMap<i64, EveCorporationModel>, HashMap<i64, i32>), AppError> {
        if corporations_map.is_empty() {
            return Ok((HashMap::new(), HashMap::new()));
        }

        let corporation_repo = CorporationRepository::new(txn);

        let corporation_relations = corporations_map.into_iter().map(|(corporation_id, corporation)| {
            let faction_record_id = corporation.faction_id.and_then(|faction_id| {
                match self.factions_record_id_map.get(&faction_id) {
                    Some(id) => Some(*id),
                    None => {
                        tracing::warn!(
                            faction_id = %faction_id,
                            corporation_id = %corporation_id,
                            "Failed to find faction record ID in database; corporation will have no related faction"
                        );
                        None
                    }
                }
            });

            let alliance_record_id = corporation.alliance_id.and_then(|alliance_id| {
                match self.alliances_record_id_map.get(&alliance_id) {
                    Some(id) => Some(*id),
                    None => {
                        tracing::warn!(
                            alliance_id = %alliance_id,
                            corporation_id = %corporation_id,
                            "Failed to find alliance record ID in database; corporation will have no related alliance"
                        );
                        None
                    }
                }
            });

            (corporation_id, corporation, alliance_record_id, faction_record_id)
        }).collect();

        let stored_corporations = corporation_repo.upsert_many(corporation_relations).await?;

        let record_id_map = stored_corporations
            .iter()
            .map(|c| (c.corporation_id, c.id))
            .collect();

        let corporations_map = stored_corporations
            .into_iter()
            .map(|c| (c.corporation_id, c))
            .collect();

        Ok((corporations_map, record_id_map))
    }

    /// Stores character entities to the database with corporation and faction relationships.
    ///
    /// Upserts all fetched characters, linking them to their corporations and factions if present.
    /// Returns empty maps immediately if no characters are provided.
    /// Characters without resolvable corporations are skipped with error logs, as corporations
    /// are required for character records.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to use
    /// - `characters_map` - Map of character IDs to character ESI data (may be empty)
    ///
    /// # Returns
    /// - `Ok((entity_map, record_id_map))` - Tuple containing:
    ///   - `HashMap<i64, EveCharacterModel>` - Map of EVE character IDs to stored database models
    ///   - `HashMap<i64, i32>` - Map of EVE character IDs to database record IDs
    /// - `Err(AppError::Database)` - Database operation failed
    pub(super) async fn store_characters(
        &self,
        txn: &DatabaseTransaction,
        characters_map: HashMap<i64, Character>,
    ) -> Result<(HashMap<i64, EveCharacterModel>, HashMap<i64, i32>), AppError> {
        if characters_map.is_empty() {
            return Ok((HashMap::new(), HashMap::new()));
        }

        let character_repo = CharacterRepository::new(txn);

        let character_relations = characters_map.into_iter().filter_map(|(character_id, character)| {
            let faction_record_id = character.faction_id.and_then(|faction_id| {
                match self.factions_record_id_map.get(&faction_id) {
                    Some(id) => Some(*id),
                    None => {
                        tracing::warn!(
                            faction_id = %faction_id,
                            character_id = %character_id,
                            "Failed to find faction record ID in database; character will have no related faction"
                        );
                        None
                    }
                }
            });

            let corporation_record_id =
                match self.corporations_record_id_map.get(&character.corporation_id) {
                    Some(id) => *id,
                    None => {
                        tracing::error!(
                            corporation_id = %character.corporation_id,
                            character_id = %character_id,
                            "Failed to find corporation record ID in database; skipping saving character to database due to missing corporation"
                        );

                        return None; // Skip this character
                    }
                };

            Some((character_id, character, corporation_record_id, faction_record_id))
        }).collect();

        let stored_characters = character_repo.upsert_many(character_relations).await?;

        let record_id_map = stored_characters
            .iter()
            .map(|c| (c.character_id, c.id))
            .collect();

        let characters_map = stored_characters
            .into_iter()
            .map(|c| (c.character_id, c))
            .collect();

        Ok((characters_map, record_id_map))
    }
}
