use chrono::Utc;
use sea_orm::{ActiveValue, EntityTrait};

use crate::{error::TestError, TestSetup};

impl TestSetup {
    pub async fn insert_mock_user(
        &self,
        main_character_id: i32,
    ) -> Result<entity::bifrost_user::Model, TestError> {
        Ok(
            entity::prelude::BifrostUser::insert(entity::bifrost_user::ActiveModel {
                main_character_id: ActiveValue::Set(main_character_id),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            })
            .exec_with_returning(&self.state.db)
            .await?,
        )
    }
}
