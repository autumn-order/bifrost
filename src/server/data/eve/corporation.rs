use chrono::Utc;
use eve_esi::model::corporation::Corporation;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
};

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

    /// Get a corporation from database using EVE Online corporation ID
    pub async fn get_by_corporation_id(
        &self,
        corporation_id: i64,
    ) -> Result<Option<entity::eve_corporation::Model>, DbErr> {
        entity::prelude::EveCorporation::find()
            .filter(entity::eve_corporation::Column::CorporationId.eq(corporation_id))
            .one(self.db)
            .await
    }
}

#[cfg(test)]
mod tests {

    mod create {
        use crate::server::data::eve::corporation::CorporationRepository;

        use bifrost_test_utils::{error::TestError, test_setup, TestSetup};

        // Expect Ok when inserting a corporation with both an alliance & faction ID
        #[tokio::test]
        async fn returns_success_for_corporation_with_alliance_and_faction() -> Result<(), TestError>
        {
            let test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let faction_model = test.insert_mock_faction(1).await?;
            let alliance_model = test.insert_mock_alliance(1, None).await?;
            let (corporation_id, corporation) = test.with_mock_corporation(
                1,
                Some(alliance_model.alliance_id),
                Some(faction_model.faction_id),
            );

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .create(
                    corporation_id,
                    corporation,
                    Some(alliance_model.id),
                    Some(faction_model.id),
                )
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let created = result.unwrap();
            let (corporation_id, corporation) = test.with_mock_corporation(
                1,
                Some(alliance_model.alliance_id),
                Some(faction_model.faction_id),
            );
            assert_eq!(created.corporation_id, corporation_id,);
            assert_eq!(created.name, corporation.name);
            assert_eq!(created.alliance_id, Some(alliance_model.id),);
            assert_eq!(created.faction_id, Some(faction_model.id));

            Ok(())
        }

        /// Expect Ok when inserting a corporation with only a faction ID
        #[tokio::test]
        async fn returns_success_for_corporation_with_faction() -> Result<(), TestError> {
            let test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let faction_model = test.insert_mock_faction(1).await?;
            let (corporation_id, corporation) =
                test.with_mock_corporation(1, None, Some(faction_model.faction_id));

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .create(corporation_id, corporation, None, Some(faction_model.id))
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.alliance_id, None);
            assert_eq!(created.faction_id, Some(faction_model.id));

            Ok(())
        }

        /// Should succeed when inserting corporation into table without a faction or alliance ID
        #[tokio::test]
        async fn returns_success_for_corporation_without_alliance_or_faction(
        ) -> Result<(), TestError> {
            let test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let (corporation_id, corporation) = test.with_mock_corporation(1, None, None);

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .create(corporation_id, corporation, None, None)
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.alliance_id, None);
            assert_eq!(created.faction_id, None);

            Ok(())
        }
    }

    mod get_by_corporation_id {
        use bifrost_test_utils::{error::TestError, test_setup, TestSetup};

        use crate::server::data::eve::corporation::CorporationRepository;

        /// Expect Some when getting corporation present in table
        #[tokio::test]
        async fn returns_success_with_existing_corporation() -> Result<(), TestError> {
            let test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let (corporation_id, corporation) = test.with_mock_corporation(1, None, None);
            let _ = test
                .insert_mock_corporation(corporation_id, corporation, None, None)
                .await?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo.get_by_corporation_id(corporation_id).await;

            assert!(result.is_ok());
            let corporation_option = result.unwrap();
            assert!(corporation_option.is_some());

            Ok(())
        }

        /// Expect None when getting corporation not present in table
        #[tokio::test]
        async fn returns_none_with_non_existant_corporation() -> Result<(), TestError> {
            let test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let corporation_id = 1;
            let result = corporation_repo.get_by_corporation_id(corporation_id).await;

            assert!(result.is_ok());
            let corporation_option = result.unwrap();
            assert!(corporation_option.is_none());

            Ok(())
        }

        /// Expect Error when required tables haven't been created
        #[tokio::test]
        async fn returns_error_with_missing_tables() -> Result<(), TestError> {
            let test = test_setup!()?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let corporation_id = 1;
            let result = corporation_repo.get_by_corporation_id(corporation_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
