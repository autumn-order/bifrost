//! Character repository for EVE Online character data management.
//!
//! This module provides the `CharacterRepository` for managing character records from
//! EVE Online's ESI API.

use crate::server::model::db::EveCharacterModel;
use chrono::Utc;
use eve_esi::model::character::Character;
use migration::{CaseStatement, Expr, OnConflict};
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect,
};

/// Repository for managing EVE Online character records in the database.
///
/// Provides operations for upserting character data from ESI, retrieving character
/// record IDs, updating corporation and faction affiliations, and mapping between
/// EVE character IDs and internal database IDs.
pub struct CharacterRepository<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> CharacterRepository<'a, C> {
    /// Creates a new instance of CharacterRepository.
    ///
    /// Constructs a repository for managing EVE character records in the database.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    ///
    /// # Returns
    /// - `CharacterRepository` - New repository instance
    pub fn new(db: &'a C) -> Self {
        Self { db }
    }

    /// Inserts or updates multiple character records from ESI data.
    ///
    /// Creates new character records or updates existing ones based on character_id.
    /// On conflict, updates all character fields except created_at. Requires corporation_id
    /// and accepts optional faction_id for characters with faction affiliations.
    ///
    /// # Arguments
    /// - `characters` - Vector of tuples containing (character_id, ESI character data, corporation_id, optional faction_id)
    ///
    /// # Returns
    /// - `Ok(Vec<EveCharacter>)` - The created or updated character records
    /// - `Err(DbErr)` - Database operation failed or foreign key constraint violated
    pub async fn upsert_many(
        &self,
        characters: Vec<(i64, Character, i32, Option<i32>)>,
    ) -> Result<Vec<EveCharacterModel>, DbErr> {
        let characters =
            characters
                .into_iter()
                .map(|(character_id, character, corporation_id, faction_id)| {
                    entity::eve_character::ActiveModel {
                        character_id: ActiveValue::Set(character_id),
                        corporation_id: ActiveValue::Set(corporation_id),
                        faction_id: ActiveValue::Set(faction_id),
                        birthday: ActiveValue::Set(character.birthday.naive_utc()),
                        bloodline_id: ActiveValue::Set(character.bloodline_id),
                        description: ActiveValue::Set(character.description),
                        gender: ActiveValue::Set(character.gender),
                        name: ActiveValue::Set(character.name),
                        race_id: ActiveValue::Set(character.race_id),
                        security_status: ActiveValue::Set(character.security_status),
                        title: ActiveValue::Set(character.title),
                        created_at: ActiveValue::Set(Utc::now().naive_utc()),
                        info_updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                        affiliation_updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                        ..Default::default()
                    }
                });

        entity::prelude::EveCharacter::insert_many(characters)
            .on_conflict(
                OnConflict::column(entity::eve_character::Column::CharacterId)
                    .update_columns([
                        entity::eve_character::Column::CorporationId,
                        entity::eve_character::Column::FactionId,
                        entity::eve_character::Column::Birthday,
                        entity::eve_character::Column::BloodlineId,
                        entity::eve_character::Column::Description,
                        entity::eve_character::Column::Gender,
                        entity::eve_character::Column::Name,
                        entity::eve_character::Column::RaceId,
                        entity::eve_character::Column::SecurityStatus,
                        entity::eve_character::Column::Title,
                        entity::eve_character::Column::InfoUpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(self.db)
            .await
    }

    /// Retrieves internal database record IDs for EVE character IDs.
    ///
    /// Maps EVE Online character IDs to their corresponding internal database record IDs.
    /// Returns only entries that exist in the database.
    ///
    /// # Arguments
    /// - `character_ids` - Slice of EVE character IDs to look up
    ///
    /// # Returns
    /// - `Ok(Vec<(i32, i64)>)` - List of (record_id, character_id) tuples for found characters
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_record_ids_by_character_ids(
        &self,
        character_ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, DbErr> {
        entity::prelude::EveCharacter::find()
            .select_only()
            .column(entity::eve_character::Column::Id)
            .column(entity::eve_character::Column::CharacterId)
            .filter(entity::eve_character::Column::CharacterId.is_in(character_ids.iter().copied()))
            .into_tuple::<(i32, i64)>()
            .all(self.db)
            .await
    }

    /// Updates corporation and faction affiliations for multiple characters.
    ///
    /// Performs bulk updates of character affiliations using CASE statements for efficient
    /// batch processing. Updates are performed in batches of 100. Silently skips characters
    /// that don't exist in the database.
    ///
    /// # Arguments
    /// - `characters` - Vector of tuples containing (character_id, corporation_id, optional faction_id)
    ///
    /// # Returns
    /// - `Ok(())` - All updates completed successfully (including empty input)
    /// - `Err(DbErr)` - Database operation failed or foreign key constraint violated
    ///
    /// # Notes
    /// - Corporation IDs must exist in the eve_corporation table due to foreign key constraint
    /// - Faction IDs must exist in the eve_faction table due to foreign key constraint
    /// - Characters that don't exist will be silently skipped
    /// - For transactional behavior, pass a transaction as the connection
    pub async fn update_affiliations(
        &self,
        characters: Vec<(i32, i32, Option<i32>)>, // (character_id, corporation_id, faction_id)
    ) -> Result<(), DbErr> {
        if characters.is_empty() {
            return Ok(());
        }

        const BATCH_SIZE: usize = 100;

        for batch in characters.chunks(BATCH_SIZE) {
            let mut corp_case_stmt = CaseStatement::new();
            let mut faction_case_stmt = CaseStatement::new();
            let character_ids: Vec<i32> = batch.iter().map(|(id, _, _)| *id).collect();

            for (character_id, corporation_id, faction_id) in batch {
                corp_case_stmt = corp_case_stmt.case(
                    entity::eve_character::Column::Id.eq(*character_id),
                    Expr::value(*corporation_id),
                );

                faction_case_stmt = faction_case_stmt.case(
                    entity::eve_character::Column::Id.eq(*character_id),
                    Expr::value(*faction_id),
                );
            }

            entity::prelude::EveCharacter::update_many()
                .col_expr(
                    entity::eve_character::Column::CorporationId,
                    Expr::value(corp_case_stmt),
                )
                .col_expr(
                    entity::eve_character::Column::FactionId,
                    Expr::value(faction_case_stmt),
                )
                .col_expr(
                    entity::eve_character::Column::AffiliationUpdatedAt,
                    Expr::value(Utc::now().naive_utc()),
                )
                .filter(entity::eve_character::Column::Id.is_in(character_ids))
                .exec(self.db)
                .await?;
        }

        Ok(())
    }
}
