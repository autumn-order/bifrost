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
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Schema};

    use crate::server::{
        data::eve::faction::FactionRepository,
        util::test::{eve::mock::mock_faction, setup::test_setup},
    };

    async fn setup() -> Result<DatabaseConnection, DbErr> {
        let test = test_setup().await;

        let db = test.state.db;
        let schema = Schema::new(DbBackend::Sqlite);

        let stmts = vec![
            schema.create_table_from_entity(entity::prelude::EveFaction),
            schema.create_table_from_entity(entity::prelude::EveAlliance),
        ];

        for stmt in stmts {
            db.execute(&stmt).await?;
        }

        Ok(db)
    }

    /// Inserts a mock faction for foreign key dependencies
    async fn insert_foreign_key_dependencies(
        db: &DatabaseConnection,
    ) -> entity::eve_faction::Model {
        let faction_repo = FactionRepository::new(&db);

        let faction = faction_repo
            .upsert_many(vec![mock_faction()])
            .await
            .unwrap()
            .first()
            .unwrap()
            .to_owned();

        faction
    }

    mod create_alliance_tests {
        use crate::server::{
            data::eve::alliance::{
                tests::{insert_foreign_key_dependencies, setup},
                AllianceRepository,
            },
            util::test::eve::mock::mock_alliance,
        };

        /// Should succeed when inserting alliance into table with a faction ID
        #[tokio::test]
        async fn create_alliance() {
            let db = setup().await.unwrap();
            let faction = insert_foreign_key_dependencies(&db).await;

            let alliance_repo = AllianceRepository::new(&db);

            let alliance_id = 1;
            let alliance = mock_alliance(Some(faction.faction_id));
            let result = alliance_repo
                .create(alliance_id, alliance, Some(faction.id))
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let created = result.unwrap();

            // Need to create mock alliance again as eve_esi::model::alliance::Alliance does not implement Clone
            // - An issue will need to be made on the eve_esi repo about this
            let alliance = mock_alliance(Some(faction.faction_id));

            assert_eq!(created.alliance_id, alliance_id, "alliance_id mismatch");
            assert_eq!(created.name, alliance.name, "name mismatch");
            assert_eq!(created.faction_id, Some(faction.id), "faction_id mismatch");
        }

        /// Should succeed when inserting alliance into table without a faction ID
        #[tokio::test]
        async fn create_alliance_no_faction() {
            let db = setup().await.unwrap();

            let alliance_repo = AllianceRepository::new(&db);

            let alliance_id = 1;
            let alliance = mock_alliance(None);
            let result = alliance_repo.create(alliance_id, alliance, None).await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let created = result.unwrap();

            assert_eq!(created.faction_id, None);
        }
    }

    mod get_by_alliance_id_tests {
        use sea_orm::DbErr;

        use crate::server::{
            data::eve::alliance::{tests::setup, AllianceRepository},
            util::test::{eve::mock::mock_alliance, setup::test_setup},
        };

        /// Expect Some when getting existing alliance from table
        #[tokio::test]
        async fn test_get_by_alliance_id_some() -> Result<(), DbErr> {
            let db = setup().await.unwrap();

            let alliance_repo = AllianceRepository::new(&db);

            let alliance_id = 1;
            let alliance = mock_alliance(None);
            let existing_alliance = alliance_repo.create(alliance_id, alliance, None).await?;

            let result = alliance_repo.get_by_alliance_id(alliance_id).await;

            assert!(result.is_ok());
            let alliance_option = result.unwrap();

            assert!(alliance_option.is_some());
            let alliance = alliance_option.unwrap();

            assert_eq!(alliance.alliance_id, existing_alliance.alliance_id);
            assert_eq!(alliance.id, existing_alliance.id);

            Ok(())
        }

        /// Expect None when getting alliance from table that does not exist
        #[tokio::test]
        async fn test_get_by_alliance_id_none() -> Result<(), DbErr> {
            let db = setup().await.unwrap();

            let alliance_repo = AllianceRepository::new(&db);

            let alliance_id = 1;
            let result = alliance_repo.get_by_alliance_id(alliance_id).await;

            assert!(result.is_ok());
            let alliance_option = result.unwrap();

            assert!(alliance_option.is_none());

            Ok(())
        }

        /// Expect Error when getting alliance from table that has not been created
        #[tokio::test]
        async fn test_get_by_alliance_id_error() -> Result<(), DbErr> {
            // Use setup function that doesn't create alliance table to cause error
            let test = test_setup().await;

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let alliance_id = 1;
            let result = alliance_repo.get_by_alliance_id(alliance_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
