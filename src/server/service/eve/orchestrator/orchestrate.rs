use std::collections::HashMap;

use entity::{
    eve_alliance::Model as EveAllianceModel, eve_character::Model as EveCharacterModel,
    eve_corporation::Model as EveCorporationModel, eve_faction::Model as EveFactionModel,
};
use sea_orm::DatabaseTransaction;

use super::{EveEntityOrchestrator, FactionFetchState};
use crate::server::error::AppError;

impl EveEntityOrchestrator {
    /// Orchestrates the complete faction storage workflow.
    ///
    /// Takes ownership of the faction state, stores it, and updates the record ID map.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveFactionModel>)` - Map of stored faction models
    /// - `Err(AppError)` - Database operation failed
    pub(super) async fn orchestrate_faction_storage(
        &mut self,
        txn: &DatabaseTransaction,
    ) -> Result<HashMap<i64, EveFactionModel>, AppError> {
        let factions_state = std::mem::replace(&mut self.factions, FactionFetchState::NotRequested);
        let (stored_factions_record_map, factions_record_id_map) =
            self.store_factions(txn, factions_state).await?;
        self.factions_record_id_map.extend(factions_record_id_map);

        Ok(stored_factions_record_map)
    }

    /// Orchestrates the complete alliance storage workflow.
    ///
    /// Takes ownership of the alliances map, stores them, and updates the record ID map.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveAllianceModel>)` - Map of stored alliance models
    /// - `Err(AppError)` - Database operation failed
    pub(super) async fn orchestrate_alliance_storage(
        &mut self,
        txn: &DatabaseTransaction,
    ) -> Result<HashMap<i64, EveAllianceModel>, AppError> {
        let alliances_map = std::mem::take(&mut self.alliances_map);
        let (stored_alliances_record_map, alliances_record_id_map) =
            self.store_alliances(txn, alliances_map).await?;
        self.alliances_record_id_map.extend(alliances_record_id_map);

        Ok(stored_alliances_record_map)
    }

    /// Orchestrates the complete corporation storage workflow.
    ///
    /// Takes ownership of the corporations map, stores them, and updates the record ID map.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveCorporationModel>)` - Map of stored corporation models
    /// - `Err(AppError)` - Database operation failed
    pub(super) async fn orchestrate_corporation_storage(
        &mut self,
        txn: &DatabaseTransaction,
    ) -> Result<HashMap<i64, EveCorporationModel>, AppError> {
        let corporations_map = std::mem::take(&mut self.corporations_map);
        let (stored_corporations_record_map, corporations_record_id_map) =
            self.store_corporations(txn, corporations_map).await?;
        self.corporations_record_id_map
            .extend(corporations_record_id_map);

        Ok(stored_corporations_record_map)
    }

    /// Orchestrates the complete character storage workflow.
    ///
    /// Takes ownership of the characters map, stores them, and updates the record ID map.
    ///
    /// # Returns
    /// - `Ok(HashMap<i64, EveCharacterModel>)` - Map of stored character models
    /// - `Err(AppError)` - Database operation failed
    pub(super) async fn orchestrate_character_storage(
        &mut self,
        txn: &DatabaseTransaction,
    ) -> Result<HashMap<i64, EveCharacterModel>, AppError> {
        let characters_map = std::mem::take(&mut self.characters_map);
        let (stored_characters_record_map, characters_record_id_map) =
            self.store_characters(txn, characters_map).await?;
        self.characters_record_id_map
            .extend(characters_record_id_map);

        Ok(stored_characters_record_map)
    }
}
