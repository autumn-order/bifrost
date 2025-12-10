//! Alliance repository for EVE Online alliance data management.
//!
//! This module provides the `AllianceRepository` for managing alliance records from
//! EVE Online's ESI API.

use crate::server::model::db::EveAllianceModel;
use chrono::Utc;
use eve_esi::model::alliance::Alliance;
use migration::OnConflict;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter,
    QuerySelect,
};

/// Repository for managing EVE Online alliance records in the database.
///
/// Provides operations for upserting alliance data from ESI, retrieving alliance
/// record IDs, and mapping between EVE alliance IDs and internal database IDs.
pub struct AllianceRepository<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> AllianceRepository<'a, C> {
    /// Creates a new instance of AllianceRepository.
    ///
    /// Constructs a repository for managing EVE alliance records in the database.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    ///
    /// # Returns
    /// - `AllianceRepository` - New repository instance
    pub fn new(db: &'a C) -> Self {
        Self { db }
    }

    /// Inserts or updates multiple alliance records from ESI data.
    ///
    /// Creates new alliance records or updates existing ones based on alliance_id.
    /// On conflict, updates all alliance fields except created_at. Accepts optional
    /// faction_id for alliances associated with NPC factions.
    ///
    /// # Arguments
    /// - `alliances` - Vector of tuples containing (alliance_id, ESI alliance data, optional faction_id)
    ///
    /// # Returns
    /// - `Ok(Vec<EveAlliance>)` - The created or updated alliance records
    /// - `Err(DbErr)` - Database operation failed or foreign key constraint violated
    pub async fn upsert_many(
        &self,
        alliances: Vec<(i64, Alliance, Option<i32>)>,
    ) -> Result<Vec<EveAllianceModel>, DbErr> {
        let alliances = alliances
            .into_iter()
            .map(
                |(alliance_id, alliance, faction_id)| entity::eve_alliance::ActiveModel {
                    alliance_id: ActiveValue::Set(alliance_id),
                    faction_id: ActiveValue::Set(faction_id),
                    creator_corporation_id: ActiveValue::Set(alliance.creator_corporation_id),
                    executor_corporation_id: ActiveValue::Set(alliance.executor_corporation_id),
                    creator_id: ActiveValue::Set(alliance.creator_id),
                    date_founded: ActiveValue::Set(alliance.date_founded.naive_utc()),
                    name: ActiveValue::Set(alliance.name),
                    ticker: ActiveValue::Set(alliance.ticker),
                    created_at: ActiveValue::Set(Utc::now().naive_utc()),
                    updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                    ..Default::default()
                },
            );

        entity::prelude::EveAlliance::insert_many(alliances)
            .on_conflict(
                OnConflict::column(entity::eve_alliance::Column::AllianceId)
                    .update_columns([
                        entity::eve_alliance::Column::FactionId,
                        entity::eve_alliance::Column::CreatorCorporationId,
                        entity::eve_alliance::Column::ExecutorCorporationId,
                        entity::eve_alliance::Column::CreatorId,
                        entity::eve_alliance::Column::DateFounded,
                        entity::eve_alliance::Column::Name,
                        entity::eve_alliance::Column::Ticker,
                        entity::eve_alliance::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(self.db)
            .await
    }

    /// Retrieves internal database record IDs for EVE alliance IDs.
    ///
    /// Maps EVE Online alliance IDs to their corresponding internal database record IDs.
    /// Returns only entries that exist in the database.
    ///
    /// # Arguments
    /// - `alliance_ids` - Slice of EVE alliance IDs to look up
    ///
    /// # Returns
    /// - `Ok(Vec<(i32, i64)>)` - List of (record_id, alliance_id) tuples for found alliances
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_record_ids_by_alliance_ids(
        &self,
        alliance_ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, DbErr> {
        entity::prelude::EveAlliance::find()
            .select_only()
            .column(entity::eve_alliance::Column::Id)
            .column(entity::eve_alliance::Column::AllianceId)
            .filter(entity::eve_alliance::Column::AllianceId.is_in(alliance_ids.iter().copied()))
            .into_tuple::<(i32, i64)>()
            .all(self.db)
            .await
    }

    /// Finds an alliance by its EVE Online alliance ID.
    ///
    /// Retrieves the alliance record from the database using the EVE alliance ID.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID to look up
    ///
    /// # Returns
    /// - `Ok(Some(EveAlliance))` - Alliance found
    /// - `Ok(None)` - Alliance not found in database
    /// - `Err(DbErr)` - Database query failed
    pub async fn find_by_eve_id(
        &self,
        alliance_id: i64,
    ) -> Result<Option<EveAllianceModel>, DbErr> {
        entity::prelude::EveAlliance::find()
            .filter(entity::eve_alliance::Column::AllianceId.eq(alliance_id))
            .one(self.db)
            .await
    }

    /// Updates the updated_at timestamp for an alliance to the current time.
    ///
    /// Sets the updated_at field to the current UTC timestamp for the specified
    /// alliance. This is used to mark when alliance information was last refreshed,
    /// even if no data changed.
    ///
    /// # Arguments
    /// - `alliance_id` - Internal database record ID of the alliance to update
    ///
    /// # Returns
    /// - `Ok(EveAlliance)` - Updated alliance record with new timestamp
    /// - `Err(DbErr)` - Database operation failed or alliance not found
    pub async fn update_info_timestamp(&self, alliance_id: i32) -> Result<EveAllianceModel, DbErr> {
        let alliance = entity::prelude::EveAlliance::find_by_id(alliance_id)
            .one(self.db)
            .await?
            .ok_or(DbErr::RecordNotFound(format!(
                "Alliance with id {} not found",
                alliance_id
            )))?;

        let mut active_model: entity::eve_alliance::ActiveModel = alliance.into();
        active_model.updated_at = ActiveValue::Set(Utc::now().naive_utc());

        active_model.update(self.db).await
    }
}
