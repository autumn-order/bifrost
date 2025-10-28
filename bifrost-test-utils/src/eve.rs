use chrono::{DateTime, Utc};
use eve_esi::model::{alliance::Alliance, universe::Faction};
use sea_orm::{ActiveValue, EntityTrait};

use crate::{error::TestError, setup::TestSetup};

impl TestSetup {
    pub fn with_mock_faction(&self) -> Faction {
        Faction {
            corporation_d: Some(0),
            description: "string".to_string(),
            faction_id: 0,
            is_unique: true,
            militia_corporation_id: Some(0),
            name: "string".to_string(),
            size_factor: 0.0,
            solar_system_id: Some(0),
            station_count: 0,
            station_system_count: 0,
        }
    }

    pub fn with_mock_alliance(&self, alliance_id: i64, faction_id: Option<i64>) -> (i64, Alliance) {
        (
            alliance_id,
            Alliance {
                creator_corporation_id: 98784257,
                creator_id: 2114794365,
                faction_id: faction_id,
                date_founded: DateTime::parse_from_rfc3339("2024-09-25T06:25:58Z")
                    .unwrap()
                    .with_timezone(&Utc),
                executor_corporation_id: Some(98787881),
                name: "Autumn.".to_string(),
                ticker: "AUTMN".to_string(),
            },
        )
    }

    pub async fn insert_mock_faction(
        &self,
        faction: Faction,
    ) -> Result<entity::eve_faction::Model, TestError> {
        Ok(
            entity::prelude::EveFaction::insert(entity::eve_faction::ActiveModel {
                faction_id: ActiveValue::Set(faction.faction_id),
                corporation_id: ActiveValue::Set(faction.corporation_d),
                militia_corporation_id: ActiveValue::Set(faction.militia_corporation_id),
                description: ActiveValue::Set(faction.description),
                is_unique: ActiveValue::Set(faction.is_unique),
                name: ActiveValue::Set(faction.name),
                size_factor: ActiveValue::Set(faction.size_factor),
                solar_system_id: ActiveValue::Set(faction.solar_system_id),
                station_count: ActiveValue::Set(faction.faction_id),
                station_system_count: ActiveValue::Set(faction.faction_id),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            })
            .exec_with_returning(&self.state.db)
            .await?,
        )
    }

    pub async fn insert_mock_alliance(
        &self,
        alliance_id: i64,
        alliance: Alliance,
        faction_id: Option<i32>,
    ) -> Result<entity::eve_alliance::Model, TestError> {
        Ok(
            entity::prelude::EveAlliance::insert(entity::eve_alliance::ActiveModel {
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
            })
            .exec_with_returning(&self.state.db)
            .await?,
        )
    }
}
