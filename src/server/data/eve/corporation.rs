use chrono::Utc;
use eve_esi::model::corporation::Corporation;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, DbErr};

pub struct CorporationRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> CorporationRepository<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        corporation_id: i64,
        corporation: Corporation,
        alliance_id: Option<i32>,
        faction_id: Option<i32>,
    ) -> Result<entity::eve_corporation::Model, DbErr> {
        let date_founded = match corporation.date_founded {
            Some(date) => Some(date.naive_utc()),
            None => None,
        };

        let corporation = entity::eve_corporation::ActiveModel {
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
            updated_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        corporation.insert(self.db).await
    }
}
