use chrono::Utc;
use eve_esi::model::corporation::Corporation;
use migration::{CaseStatement, Expr, OnConflict};
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect,
};

pub struct CorporationRepository<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> CorporationRepository<'a, C> {
    pub fn new(db: &'a C) -> Self {
        Self { db }
    }

    pub async fn upsert_many(
        &self,
        corporations: Vec<(i64, Corporation, Option<i32>, Option<i32>)>,
    ) -> Result<Vec<entity::eve_corporation::Model>, DbErr> {
        let corporations = corporations.into_iter().map(
            |(corporation_id, corporation, alliance_id, faction_id)| {
                let date_founded = match corporation.date_founded {
                    Some(date) => Some(date.naive_utc()),
                    None => None,
                };

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

    pub async fn get_entry_ids_by_corporation_ids(
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

    /// Updates a list of corporations to the provided alliance IDs
    ///
    /// # Arguments
    /// - `corporations`: Vector of a tuple containing corporation ID to update and optional alliance ID
    ///
    /// # Notes
    /// - Alliance IDs must exist in the eve_alliance table due to foreign key constraint
    /// - Corporations that don't exist will be silently skipped
    /// - If you need transactional behavior, pass a transaction as the connection
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
}
