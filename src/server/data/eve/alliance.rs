use chrono::Utc;
use eve_esi::model::alliance::Alliance;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
};

pub struct AllianceRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> AllianceRepository<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// Create an alliance using its ESI model
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

    /// Get an alliance using its EVE Online alliance ID
    pub async fn get_by_alliance_id(
        &self,
        alliance_id: i64,
    ) -> Result<Option<entity::eve_alliance::Model>, DbErr> {
        entity::prelude::EveAlliance::find()
            .filter(entity::eve_alliance::Column::AllianceId.eq(alliance_id))
            .one(self.db)
            .await
    }
}

#[cfg(test)]
mod tests {

    mod create {
        use bifrost_test_utils::{error::TestError, test_setup, TestSetup};

        use crate::server::data::eve::alliance::AllianceRepository;

        /// Expect Ok when creating alliance entry with a related faction
        #[tokio::test]
        async fn returns_success_creating_alliance_with_faction_id() -> Result<(), TestError> {
            let test = test_setup!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let faction = test.with_mock_faction();
            let faction_model = test.insert_mock_faction(&faction).await?;
            let (alliance_id, alliance) =
                test.with_mock_alliance(1, Some(faction_model.faction_id));

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let result = alliance_repo
                .create(alliance_id, alliance, Some(faction_model.id))
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();

            let (alliance_id, alliance) =
                test.with_mock_alliance(1, Some(faction_model.faction_id));
            assert_eq!(created.alliance_id, alliance_id);
            assert_eq!(created.name, alliance.name);
            assert_eq!(created.faction_id, Some(faction_model.id));

            Ok(())
        }

        /// Expect Ok when creating alliance entry without faction ID
        #[tokio::test]
        async fn returns_success_creating_alliance_without_faction() -> Result<(), TestError> {
            let test = test_setup!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id, alliance) = test.with_mock_alliance(1, None);

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let result = alliance_repo.create(alliance_id, alliance, None).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.faction_id, None);

            Ok(())
        }
    }

    mod get_by_alliance_id {
        use bifrost_test_utils::{error::TestError, test_setup, TestSetup};

        use crate::server::data::eve::alliance::AllianceRepository;

        /// Expect Some when getting existing alliance from table
        #[tokio::test]
        async fn returns_some_with_existing_alliance() -> Result<(), TestError> {
            let test = test_setup!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id, alliance) = test.with_mock_alliance(1, None);
            let alliance_model = test
                .insert_mock_alliance(alliance_id, alliance, None)
                .await?;

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let result = alliance_repo.get_by_alliance_id(alliance_id).await;

            assert!(result.is_ok());
            let alliance_option = result.unwrap();
            assert!(alliance_option.is_some());
            let alliance = alliance_option.unwrap();
            assert_eq!(alliance.alliance_id, alliance_model.alliance_id);
            assert_eq!(alliance.id, alliance_model.id);

            Ok(())
        }

        /// Expect None when getting alliance from table that does not exist
        #[tokio::test]
        async fn returns_none_with_non_existant_alliance() -> Result<(), TestError> {
            let test = test_setup!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let alliance_id = 1;
            let result = alliance_repo.get_by_alliance_id(alliance_id).await;

            assert!(result.is_ok());
            let alliance_option = result.unwrap();
            assert!(alliance_option.is_none());

            Ok(())
        }

        /// Expect Error when getting alliance from table that has not been created
        #[tokio::test]
        async fn returns_error_with_missing_tables() -> Result<(), TestError> {
            let test = test_setup!()?;

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let alliance_id = 1;
            let result = alliance_repo.get_by_alliance_id(alliance_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
