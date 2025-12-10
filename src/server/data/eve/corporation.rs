//! Corporation repository for EVE Online corporation data management.
//!
//! This module provides the `CorporationRepository` for managing corporation records from
//! EVE Online's ESI API.

use crate::server::model::db::EveCorporationModel;
use chrono::Utc;
use eve_esi::model::corporation::Corporation;
use migration::{CaseStatement, Expr, OnConflict};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter,
    QuerySelect,
};

/// Repository for managing EVE Online corporation records in the database.
///
/// Provides operations for upserting corporation data from ESI, retrieving corporation
/// record IDs, updating alliance affiliations, and mapping between EVE corporation IDs
/// and internal database IDs.
pub struct CorporationRepository<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> CorporationRepository<'a, C> {
    /// Creates a new instance of CorporationRepository.
    ///
    /// Constructs a repository for managing EVE corporation records in the database.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    ///
    /// # Returns
    /// - `CorporationRepository` - New repository instance
    pub fn new(db: &'a C) -> Self {
        Self { db }
    }

    /// Inserts or updates multiple corporation records from ESI data.
    ///
    /// Creates new corporation records or updates existing ones based on corporation_id.
    /// On conflict, updates all corporation fields except created_at. Accepts optional
    /// alliance_id and faction_id for corporations with those affiliations.
    ///
    /// # Arguments
    /// - `corporations` - Vector of tuples containing (corporation_id, ESI corporation data, optional alliance_id, optional faction_id)
    ///
    /// # Returns
    /// - `Ok(Vec<EveCorporation>)` - The created or updated corporation records
    /// - `Err(DbErr)` - Database operation failed or foreign key constraint violated
    pub async fn upsert_many(
        &self,
        corporations: Vec<(i64, Corporation, Option<i32>, Option<i32>)>,
    ) -> Result<Vec<EveCorporationModel>, DbErr> {
        let corporations = corporations.into_iter().map(
            |(corporation_id, corporation, alliance_id, faction_id)| {
                let date_founded = corporation.date_founded.map(|date| date.naive_utc());

                entity::eve_corporation::ActiveModel {
                    corporation_id: ActiveValue::Set(corporation_id),
                    alliance_id: ActiveValue::Set(alliance_id),
                    faction_id: ActiveValue::Set(faction_id),
                    ceo_id: ActiveValue::Set(corporation.ceo_id),
                    creator_id: ActiveValue::Set(corporation.creator_id),
                    date_founded: ActiveValue::Set(date_founded),
                    description: ActiveValue::Set(corporation.description),
                    home_station_id: ActiveValue::Set(corporation.home_station_id),
                    member_count: ActiveValue::Set(corporation.member_count),
                    name: ActiveValue::Set(corporation.name),
                    shares: ActiveValue::Set(corporation.shares),
                    tax_rate: ActiveValue::Set(corporation.tax_rate),
                    ticker: ActiveValue::Set(corporation.ticker),
                    url: ActiveValue::Set(corporation.url),
                    war_eligible: ActiveValue::Set(corporation.war_eligible),
                    created_at: ActiveValue::Set(Utc::now().naive_utc()),
                    info_updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                    affiliation_updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                    ..Default::default()
                }
            },
        );

        entity::prelude::EveCorporation::insert_many(corporations)
            .on_conflict(
                OnConflict::column(entity::eve_corporation::Column::CorporationId)
                    .update_columns([
                        entity::eve_corporation::Column::AllianceId,
                        entity::eve_corporation::Column::FactionId,
                        entity::eve_corporation::Column::CeoId,
                        entity::eve_corporation::Column::CreatorId,
                        entity::eve_corporation::Column::DateFounded,
                        entity::eve_corporation::Column::Description,
                        entity::eve_corporation::Column::HomeStationId,
                        entity::eve_corporation::Column::MemberCount,
                        entity::eve_corporation::Column::Name,
                        entity::eve_corporation::Column::Shares,
                        entity::eve_corporation::Column::TaxRate,
                        entity::eve_corporation::Column::Ticker,
                        entity::eve_corporation::Column::Url,
                        entity::eve_corporation::Column::WarEligible,
                        entity::eve_corporation::Column::InfoUpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(self.db)
            .await
    }

    /// Retrieves internal database record IDs for EVE corporation IDs.
    ///
    /// Maps EVE Online corporation IDs to their corresponding internal database record IDs.
    /// Returns only entries that exist in the database.
    ///
    /// # Arguments
    /// - `corporation_ids` - Slice of EVE corporation IDs to look up
    ///
    /// # Returns
    /// - `Ok(Vec<(i32, i64)>)` - List of (record_id, corporation_id) tuples for found corporations
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_record_ids_by_corporation_ids(
        &self,
        corporation_ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, DbErr> {
        entity::prelude::EveCorporation::find()
            .select_only()
            .column(entity::eve_corporation::Column::Id)
            .column(entity::eve_corporation::Column::CorporationId)
            .filter(
                entity::eve_corporation::Column::CorporationId
                    .is_in(corporation_ids.iter().copied()),
            )
            .into_tuple::<(i32, i64)>()
            .all(self.db)
            .await
    }

    /// Updates alliance affiliations for multiple corporations.
    ///
    /// Performs bulk updates of corporation alliance affiliations using a CASE statement
    /// for efficient batch processing. Updates are performed in batches of 100. Silently
    /// skips corporations that don't exist in the database.
    ///
    /// # Arguments
    /// - `corporations` - Vector of tuples containing (corporation_id, optional alliance_id)
    ///
    /// # Returns
    /// - `Ok(())` - All updates completed successfully (including empty input)
    /// - `Err(DbErr)` - Database operation failed or alliance foreign key constraint violated
    ///
    /// # Notes
    /// - Alliance IDs must exist in the eve_alliance table due to foreign key constraint
    /// - Corporations that don't exist will be silently skipped
    /// - For transactional behavior, pass a transaction as the connection
    pub async fn update_affiliations(
        &self,
        corporations: Vec<(i32, Option<i32>)>, // (corporation_id, alliance_id)
    ) -> Result<(), DbErr> {
        if corporations.is_empty() {
            return Ok(());
        }

        const BATCH_SIZE: usize = 100;

        for batch in corporations.chunks(BATCH_SIZE) {
            let mut case_stmt = CaseStatement::new();
            let corporation_ids: Vec<i32> = batch.iter().map(|(id, _)| *id).collect();

            for (corp_id, alliance_id) in batch {
                case_stmt = case_stmt.case(
                    entity::eve_corporation::Column::Id.eq(*corp_id),
                    Expr::value(*alliance_id),
                );
            }

            entity::prelude::EveCorporation::update_many()
                .col_expr(
                    entity::eve_corporation::Column::AllianceId,
                    Expr::value(case_stmt),
                )
                .col_expr(
                    entity::eve_corporation::Column::AffiliationUpdatedAt,
                    Expr::value(Utc::now().naive_utc()),
                )
                .filter(entity::eve_corporation::Column::Id.is_in(corporation_ids))
                .exec(self.db)
                .await?;
        }

        Ok(())
    }

    /// Finds a corporation by its EVE Online corporation ID.
    ///
    /// Retrieves the corporation record from the database using the EVE corporation ID.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID to look up
    ///
    /// # Returns
    /// - `Ok(Some(EveCorporation))` - Corporation found
    /// - `Ok(None)` - Corporation not found in database
    /// - `Err(DbErr)` - Database query failed
    pub async fn find_by_eve_id(
        &self,
        corporation_id: i64,
    ) -> Result<Option<EveCorporationModel>, DbErr> {
        entity::prelude::EveCorporation::find()
            .filter(entity::eve_corporation::Column::CorporationId.eq(corporation_id))
            .one(self.db)
            .await
    }

    /// Updates the info_updated_at timestamp for a corporation to the current time.
    ///
    /// Sets the info_updated_at field to the current UTC timestamp for the specified
    /// corporation. This is used to mark when corporation information was last refreshed,
    /// even if no data changed.
    ///
    /// # Arguments
    /// - `corporation_id` - Internal database record ID of the corporation to update
    ///
    /// # Returns
    /// - `Ok(EveCorporation)` - Updated corporation record with new timestamp
    /// - `Err(DbErr)` - Database operation failed or corporation not found
    pub async fn update_info_timestamp(
        &self,
        corporation_id: i32,
    ) -> Result<EveCorporationModel, DbErr> {
        let corporation = entity::prelude::EveCorporation::find_by_id(corporation_id)
            .one(self.db)
            .await?
            .ok_or(DbErr::RecordNotFound(format!(
                "Corporation with id {} not found",
                corporation_id
            )))?;

        let mut active_model: entity::eve_corporation::ActiveModel = corporation.into();
        active_model.info_updated_at = ActiveValue::Set(Utc::now().naive_utc());

        active_model.update(self.db).await
    }
}
