use chrono::Utc;
use eve_esi::model::alliance::Alliance;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, DbErr};

pub struct AllianceRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> AllianceRepository<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        alliance_id: i64,
        alliance: Alliance,
        faction_id: Option<i32>,
    ) -> Result<entity::eve_alliance::Model, DbErr> {
        let alliance = entity::eve_alliance::ActiveModel {
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
        };

        alliance.insert(self.db).await
    }
}
