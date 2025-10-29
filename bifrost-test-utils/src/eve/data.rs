use chrono::Utc;
use eve_esi::model::{character::Character, corporation::Corporation};
use sea_orm::{ActiveValue, EntityTrait};

use crate::{error::TestError, setup::TestSetup};

impl TestSetup {
    pub async fn insert_mock_faction(
        &self,
        faction_id: i64,
    ) -> Result<entity::eve_faction::Model, TestError> {
        let faction = self.with_mock_faction(faction_id);

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
        faction_id: Option<i64>,
    ) -> Result<entity::eve_alliance::Model, TestError> {
        let faction_model_id = if let Some(faction_id) = faction_id {
            Some(self.insert_mock_faction(faction_id).await?.id)
        } else {
            None
        };

        let (alliance_id, alliance) = self.with_mock_alliance(alliance_id, faction_id);

        Ok(
            entity::prelude::EveAlliance::insert(entity::eve_alliance::ActiveModel {
                alliance_id: ActiveValue::Set(alliance_id),
                faction_id: ActiveValue::Set(faction_model_id),
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
