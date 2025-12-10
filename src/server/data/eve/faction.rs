//! Faction repository for EVE Online faction data management.
//!
//! This module provides the `FactionRepository` for managing faction records from
//! EVE Online's ESI API.

use crate::server::model::db::EveFactionModel;
use chrono::Utc;
use eve_esi::model::universe::Faction;
use migration::OnConflict;
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, Order, QueryFilter, QueryOrder,
    QuerySelect,
};

/// Repository for managing EVE Online faction records in the database.
///
/// Provides operations for upserting faction data from ESI, retrieving faction
/// record IDs, and querying the latest faction update timestamp.
pub struct FactionRepository<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> FactionRepository<'a, C> {
    /// Creates a new instance of FactionRepository.
    ///
    /// Constructs a repository for managing EVE faction records in the database.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    ///
    /// # Returns
    /// - `FactionRepository` - New repository instance
    pub fn new(db: &'a C) -> Self {
        Self { db }
    }

    /// Inserts or updates multiple faction records from ESI data.
    ///
    /// Creates new faction records or updates existing ones based on faction_id.
    /// On conflict, updates all faction fields except created_at.
    ///
    /// # Arguments
    /// - `factions` - Vector of ESI faction data
    ///
    /// # Returns
    /// - `Ok(Vec<EveFaction>)` - The created or updated faction records
    /// - `Err(DbErr)` - Database operation failed
    pub async fn upsert_many(&self, factions: Vec<Faction>) -> Result<Vec<EveFactionModel>, DbErr> {
        let factions = factions
            .into_iter()
            .map(|f| entity::eve_faction::ActiveModel {
                faction_id: ActiveValue::Set(f.faction_id),
                corporation_id: ActiveValue::Set(f.corporation_id),
                militia_corporation_id: ActiveValue::Set(f.militia_corporation_id),
                description: ActiveValue::Set(f.description),
                is_unique: ActiveValue::Set(f.is_unique),
                name: ActiveValue::Set(f.name),
                size_factor: ActiveValue::Set(f.size_factor),
                solar_system_id: ActiveValue::Set(f.solar_system_id),
                station_count: ActiveValue::Set(f.faction_id),
                station_system_count: ActiveValue::Set(f.faction_id),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            });

        entity::prelude::EveFaction::insert_many(factions)
            .on_conflict(
                OnConflict::column(entity::eve_faction::Column::FactionId)
                    .update_columns([
                        entity::eve_faction::Column::CorporationId,
                        entity::eve_faction::Column::MilitiaCorporationId,
                        entity::eve_faction::Column::Description,
                        entity::eve_faction::Column::IsUnique,
                        entity::eve_faction::Column::Name,
                        entity::eve_faction::Column::SizeFactor,
                        entity::eve_faction::Column::SolarSystemId,
                        entity::eve_faction::Column::StationCount,
                        entity::eve_faction::Column::StationSystemCount,
                        entity::eve_faction::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(self.db)
            .await
    }

    /// Retrieves internal database record IDs for EVE faction IDs.
    ///
    /// Maps EVE Online faction IDs to their corresponding internal database record IDs.
    /// Returns only entries that exist in the database.
    ///
    /// # Arguments
    /// - `faction_ids` - Slice of EVE faction IDs to look up
    ///
    /// # Returns
    /// - `Ok(Vec<(i32, i64)>)` - List of (record_id, faction_id) tuples for found factions
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_record_ids_by_faction_ids(
        &self,
        faction_ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, DbErr> {
        entity::prelude::EveFaction::find()
            .select_only()
            .column(entity::eve_faction::Column::Id)
            .column(entity::eve_faction::Column::FactionId)
            .filter(entity::eve_faction::Column::FactionId.is_in(faction_ids.iter().copied()))
            .into_tuple::<(i32, i64)>()
            .all(self.db)
            .await
    }

    /// Retrieves the most recently updated faction record.
    ///
    /// Fetches the faction with the latest updated_at timestamp. Useful for determining
    /// when faction data was last refreshed from ESI.
    ///
    /// # Returns
    /// - `Ok(Some(EveFaction))` - The most recently updated faction record
    /// - `Ok(None)` - No factions exist in the database
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_latest(&self) -> Result<Option<EveFactionModel>, DbErr> {
        entity::prelude::EveFaction::find()
            .order_by(entity::eve_faction::Column::UpdatedAt, Order::Desc)
            .one(self.db)
            .await
    }

    /// Updates the updated_at timestamp for all faction records.
    ///
    /// Sets the `updated_at` timestamp to the current time for all faction records
    /// in the database. Used when ESI returns 304 Not Modified to indicate the
    /// cached data is still current without updating other fields.
    ///
    /// # Returns
    /// - `Ok(())` - All faction timestamps updated successfully
    /// - `Err(DbErr)` - Database operation failed
    pub async fn update_all_timestamps(&self) -> Result<(), DbErr> {
        entity::prelude::EveFaction::update_many()
            .col_expr(
                entity::eve_faction::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now().naive_utc()),
            )
            .exec(self.db)
            .await?;

        Ok(())
    }
}
