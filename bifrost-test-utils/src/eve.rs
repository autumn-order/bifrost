use chrono::{DateTime, Utc};
use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};
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

    pub fn with_mock_corporation(
        &self,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> (i64, Corporation) {
        (
            corporation_id,
            Corporation {
                alliance_id: alliance_id,
                ceo_id: 2114794365,
                creator_id: 2114794365,
                date_founded: Some(
                    DateTime::parse_from_rfc3339("2024-10-07T21:43:09Z")
                        .unwrap()
                        .with_timezone(&Utc),
                ),
                description: None,
                home_station_id: Some(60003760),
                member_count: 21,
                name: "The Order of Autumn".to_string(),
                shares: Some(1000),
                tax_rate: 0.0,
                ticker: "F4LL.".to_string(),
                url: Some("https://autumn-order.com".to_string()),
                war_eligible: Some(true),
                faction_id: faction_id,
            },
        )
    }

    pub fn with_mock_character(
        &self,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> (i64, Character) {
        (
            character_id,
            Character {
                alliance_id: alliance_id,
                birthday: DateTime::parse_from_rfc3339("2018-12-20T16:11:54Z")
                    .unwrap()
                    .with_timezone(&Utc),
                bloodline_id: 7,
                corporation_id: corporation_id,
                description: Some("description".to_string()),
                faction_id: faction_id,
                gender: "male".to_string(),
                name: "Hyziri".to_string(),
                race_id: 8,
                security_status: Some(-0.100373643),
                title: Some("Title".to_string()),
            },
        )
    }

    pub async fn insert_mock_faction(
        &self,
        faction: &Faction,
    ) -> Result<entity::eve_faction::Model, TestError> {
        Ok(
            entity::prelude::EveFaction::insert(entity::eve_faction::ActiveModel {
                faction_id: ActiveValue::Set(faction.faction_id),
                corporation_id: ActiveValue::Set(faction.corporation_d),
                militia_corporation_id: ActiveValue::Set(faction.militia_corporation_id),
                description: ActiveValue::Set(faction.description.to_string()),
                is_unique: ActiveValue::Set(faction.is_unique),
                name: ActiveValue::Set(faction.name.to_string()),
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

    pub async fn insert_mock_corporation(
        &self,
        corporation_id: i64,
        corporation: Corporation,
        alliance_id: Option<i32>,
        faction_id: Option<i32>,
    ) -> Result<entity::eve_corporation::Model, TestError> {
        let date_founded = match corporation.date_founded {
            Some(date) => Some(date.naive_utc()),
            None => None,
        };

        Ok(
            entity::prelude::EveCorporation::insert(entity::eve_corporation::ActiveModel {
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
            })
            .exec_with_returning(&self.state.db)
            .await?,
        )
    }

    pub async fn insert_mock_character(
        &self,
        character_id: i64,
        character: Character,
        corporation_id: i32,
        faction_id: Option<i32>,
    ) -> Result<entity::eve_character::Model, TestError> {
        Ok(
            entity::prelude::EveCharacter::insert(entity::eve_character::ActiveModel {
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
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            })
            .exec_with_returning(&self.state.db)
            .await?,
        )
    }
}
