//! EVE entity database insertion utilities.
//!
//! This module provides methods for inserting EVE Online entity records into the test
//! database with automatic parent entity creation. If a parent entity is specified but
//! doesn't exist, it will be created automatically to maintain referential integrity.

use crate::model::{EveAllianceModel, EveCharacterModel, EveCorporationModel, EveFactionModel};
use chrono::Utc;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter};

use crate::{error::TestError, fixtures::eve::EveFixtures};

impl<'a> EveFixtures<'a> {
    /// Insert a mock faction into the database.
    ///
    /// Creates an EveFaction record with standard test values. If a faction with the
    /// specified ID already exists, returns the existing record instead of creating a duplicate.
    ///
    /// # Arguments
    /// - `faction_id` - The EVE Online faction ID to insert
    ///
    /// # Returns
    /// - `Ok(EveFactionModel)` - The created or existing faction record
    /// - `Err(TestError::DbErr)` - Database query or insert operation failed
    pub async fn insert_mock_faction(&self, faction_id: i64) -> Result<EveFactionModel, TestError> {
        if let Some(existing_faction) = entity::prelude::EveFaction::find()
            .filter(entity::eve_faction::Column::FactionId.eq(faction_id))
            .one(&self.setup.db)
            .await?
        {
            return Ok(existing_faction);
        }

        let faction = self.mock_faction(faction_id);

        Ok(
            entity::prelude::EveFaction::insert(entity::eve_faction::ActiveModel {
                faction_id: ActiveValue::Set(faction.faction_id),
                corporation_id: ActiveValue::Set(faction.corporation_id),
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
            .exec_with_returning(&self.setup.db)
            .await?,
        )
    }

    /// Insert a mock alliance into the database.
    ///
    /// Creates an EveAlliance record with standard test values. If a faction_id is provided,
    /// the faction will be created automatically if it doesn't exist. If an alliance with
    /// the specified ID already exists, returns the existing record.
    ///
    /// # Arguments
    /// - `alliance_id` - The EVE Online alliance ID to insert
    /// - `faction_id` - Optional faction ID the alliance belongs to
    ///
    /// # Returns
    /// - `Ok(EveAllianceModel)` - The created or existing alliance record
    /// - `Err(TestError::DbErr)` - Database query or insert operation failed
    pub async fn insert_mock_alliance(
        &self,
        alliance_id: i64,
        faction_id: Option<i64>,
    ) -> Result<EveAllianceModel, TestError> {
        if let Some(existing_alliance) = entity::prelude::EveAlliance::find()
            .filter(entity::eve_alliance::Column::AllianceId.eq(alliance_id))
            .one(&self.setup.db)
            .await?
        {
            return Ok(existing_alliance);
        }

        let faction_model_id = if let Some(faction_id) = faction_id {
            Some(self.insert_mock_faction(faction_id).await?.id)
        } else {
            None
        };

        let (alliance_id, alliance) = self.mock_alliance(alliance_id, faction_id);

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
            .exec_with_returning(&self.setup.db)
            .await?,
        )
    }

    /// Insert a mock corporation into the database.
    ///
    /// Creates an EveCorporation record with standard test values. Parent entities
    /// (alliance, faction) will be created automatically if specified and don't exist.
    /// If a corporation with the specified ID already exists, returns the existing record.
    ///
    /// # Arguments
    /// - `corporation_id` - The EVE Online corporation ID to insert
    /// - `alliance_id` - Optional alliance ID the corporation belongs to
    /// - `faction_id` - Optional faction ID the corporation belongs to
    ///
    /// # Returns
    /// - `Ok(EveCorporationModel)` - The created or existing corporation record
    /// - `Err(TestError::DbErr)` - Database query or insert operation failed
    pub async fn insert_mock_corporation(
        &self,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> Result<EveCorporationModel, TestError> {
        if let Some(existing_corporation) = entity::prelude::EveCorporation::find()
            .filter(entity::eve_corporation::Column::CorporationId.eq(corporation_id))
            .one(&self.setup.db)
            .await?
        {
            return Ok(existing_corporation);
        }

        let alliance_model_id = if let Some(alliance_id) = alliance_id {
            Some(self.insert_mock_alliance(alliance_id, None).await?.id)
        } else {
            None
        };

        let faction_model_id = if let Some(faction_id) = faction_id {
            Some(self.insert_mock_faction(faction_id).await?.id)
        } else {
            None
        };

        let (corporation_id, corporation) =
            self.mock_corporation(corporation_id, alliance_id, faction_id);

        let date_founded = match corporation.date_founded {
            Some(date) => Some(date.naive_utc()),
            None => None,
        };

        Ok(
            entity::prelude::EveCorporation::insert(entity::eve_corporation::ActiveModel {
                corporation_id: ActiveValue::Set(corporation_id),
                alliance_id: ActiveValue::Set(alliance_model_id),
                faction_id: ActiveValue::Set(faction_model_id),
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
            })
            .exec_with_returning(&self.setup.db)
            .await?,
        )
    }

    /// Insert a mock character into the database with full hierarchy.
    ///
    /// Creates an EveCharacter record with standard test values. All parent entities
    /// (corporation, alliance, faction) will be created automatically if they don't exist,
    /// ensuring the full organizational hierarchy is present in the database.
    ///
    /// # Arguments
    /// - `character_id` - The EVE Online character ID to insert
    /// - `corporation_id` - The corporation ID the character belongs to
    /// - `alliance_id` - Optional alliance ID the character belongs to
    /// - `faction_id` - Optional faction ID the character belongs to
    ///
    /// # Returns
    /// - `Ok(EveCharacterModel)` - The created character record
    /// - `Err(TestError::DbErr)` - Database query or insert operation failed
    pub async fn insert_mock_character(
        &self,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> Result<EveCharacterModel, TestError> {
        let faction_model_id = if let Some(faction_id) = faction_id {
            Some(self.insert_mock_faction(faction_id).await?.id)
        } else {
            None
        };

        let _ = if let Some(alliance_id) = alliance_id {
            Some(self.insert_mock_alliance(alliance_id, None).await?.id)
        } else {
            None
        };

        let corporation_model = self
            .insert_mock_corporation(corporation_id, None, None)
            .await?;
        let (character_id, character) =
            self.mock_character(character_id, corporation_id, alliance_id, faction_id);

        Ok(
            entity::prelude::EveCharacter::insert(entity::eve_character::ActiveModel {
                character_id: ActiveValue::Set(character_id),
                corporation_id: ActiveValue::Set(corporation_model.id),
                faction_id: ActiveValue::Set(faction_model_id),
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
            })
            .exec_with_returning(&self.setup.db)
            .await?,
        )
    }
}
