use chrono::Utc;
use sea_orm::{ActiveValue, EntityTrait};

use crate::{error::TestError, TestSetup};

impl TestSetup {
    pub async fn insert_user(
        &self,
        character_id: i32,
    ) -> Result<entity::bifrost_user::Model, TestError> {
        Ok(
            entity::prelude::BifrostUser::insert(entity::bifrost_user::ActiveModel {
                main_character_id: ActiveValue::Set(character_id),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            })
            .exec_with_returning(&self.state.db)
            .await?,
        )
    }

    pub async fn insert_character_ownership_for_user(
        &self,
        user_id: i32,
        character_id: i32,
    ) -> Result<entity::bifrost_user_character::Model, TestError> {
        Ok(entity::prelude::BifrostUserCharacter::insert(
            entity::bifrost_user_character::ActiveModel {
                user_id: ActiveValue::Set(user_id),
                character_id: ActiveValue::Set(character_id),
                owner_hash: ActiveValue::Set("owner_hash".to_string()),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            },
        )
        .exec_with_returning(&self.state.db)
        .await?)
    }

    pub async fn insert_mock_user_with_character(
        &self,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> Result<
        (
            entity::bifrost_user::Model,
            entity::bifrost_user_character::Model,
            entity::eve_character::Model,
        ),
        TestError,
    > {
        let character_model = self
            .insert_mock_character(character_id, corporation_id, alliance_id, faction_id)
            .await?;

        let user_model = self.insert_user(character_model.id).await?;

        let user_character_model = self
            .insert_character_ownership_for_user(user_model.id, character_model.id)
            .await?;

        Ok((user_model, user_character_model, character_model))
    }

    pub async fn insert_mock_character_owned_by_user(
        &self,
        user_id: i32,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> Result<
        (
            entity::bifrost_user_character::Model,
            entity::eve_character::Model,
        ),
        TestError,
    > {
        let character_model = self
            .insert_mock_character(character_id, corporation_id, alliance_id, faction_id)
            .await?;

        let user_character_model = self
            .insert_character_ownership_for_user(user_id, character_model.id)
            .await?;

        Ok((user_character_model, character_model))
    }
}
