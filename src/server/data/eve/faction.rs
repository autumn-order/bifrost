use chrono::Utc;
use eve_esi::model::universe::Faction;
use migration::OnConflict;
use sea_orm::{
    ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, Order, QueryFilter,
    QueryOrder,
};

pub struct FactionRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> FactionRepository<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn upsert_many(
        &self,
        factions: Vec<Faction>,
    ) -> Result<Vec<entity::eve_faction::Model>, DbErr> {
        let factions = factions
            .into_iter()
            .map(|f| entity::eve_faction::ActiveModel {
                faction_id: ActiveValue::Set(f.faction_id),
                corporation_id: ActiveValue::Set(f.corporation_d),
                militia_corporation_id: ActiveValue::Set(f.militia_corporation_id),
                description: ActiveValue::Set(f.description),
                is_unique: ActiveValue::Set(f.is_unique),
                name: ActiveValue::Set(f.name),
                size_factor: ActiveValue::Set(f.size_factor),
                solar_system_id: ActiveValue::Set(f.solar_system_id),
                station_count: ActiveValue::Set(f.faction_id),
                station_system_count: ActiveValue::Set(f.faction_id),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            });

        entity::prelude::EveFaction::insert_many(factions)
            .on_conflict(
                OnConflict::column(entity::eve_faction::Column::FactionId)
                    .update_columns([
                        entity::eve_faction::Column::CorporationId,
                        entity::eve_faction::Column::MilitiaCorporationId,
                        entity::eve_faction::Column::Description,
                        entity::eve_faction::Column::IsUnique,
                        entity::eve_faction::Column::Name,
                        entity::eve_faction::Column::SizeFactor,
                        entity::eve_faction::Column::SolarSystemId,
                        entity::eve_faction::Column::StationCount,
                        entity::eve_faction::Column::StationSystemCount,
                        entity::eve_faction::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(self.db)
            .await
    }

    /// Get a faction using its EVE Online faction ID
    pub async fn get_by_faction_id(
        &self,
        faction_id: i64,
    ) -> Result<Option<entity::eve_faction::Model>, DbErr> {
        entity::prelude::EveFaction::find()
            .filter(entity::eve_faction::Column::FactionId.eq(faction_id))
            .one(self.db)
            .await
    }

    /// Get the latest faction entry
    pub async fn get_latest(&self) -> Result<Option<entity::eve_faction::Model>, DbErr> {
        entity::prelude::EveFaction::find()
            .order_by(entity::eve_faction::Column::UpdatedAt, Order::Desc)
            .one(self.db)
            .await
    }
}

#[cfg(test)]
mod tests {

    mod upsert_many {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::eve::faction::FactionRepository;

        /// Expect Ok when upserting a new faction
        #[tokio::test]
        async fn upserts_new_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            let mock_faction = test.eve().with_mock_faction(1);

            let repo = FactionRepository::new(&test.state.db);
            let result = repo.upsert_many(vec![mock_faction]).await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let created_factions = result.unwrap();
            assert_eq!(created_factions.len(), 1);

            Ok(())
        }

        /// Expect Ok & update when trying to upsert an existing faction
        #[tokio::test]
        async fn updates_existing_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            let mock_faction = test.eve().with_mock_faction(1);
            let mock_faction_update = test.eve().with_mock_faction(1);

            let repo = FactionRepository::new(&test.state.db);
            let initial = repo.upsert_many(vec![mock_faction]).await?;
            let initial_entry = initial.into_iter().next().expect("no entry returned");

            let initial_created_at = initial_entry.created_at;
            let initial_updated_at = initial_entry.updated_at;

            let latest = repo.upsert_many(vec![mock_faction_update]).await?;
            let latest_entry = latest.into_iter().next().expect("no entry returned");

            // created_at should not change and updated_at should increase
            assert_eq!(latest_entry.created_at, initial_created_at);
            assert!(latest_entry.updated_at > initial_updated_at);

            Ok(())
        }
    }

    mod get_by_faction_id {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::eve::faction::FactionRepository;

        /// Expect Some when faction is present in the table
        #[tokio::test]
        async fn finds_existing_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            let faction_id = 1;
            let mock_faction = test.eve().with_mock_faction(faction_id);

            let repo = FactionRepository::new(&test.state.db);
            let initial = repo.upsert_many(vec![mock_faction]).await?;
            let initial_entry = initial.into_iter().next().expect("no entry returned");
            let result = repo.get_by_faction_id(faction_id).await?;

            assert!(result.is_some());
            let faction = result.unwrap();

            assert_eq!(initial_entry.id, faction.id);
            assert_eq!(initial_entry.faction_id, faction.faction_id);

            Ok(())
        }

        /// Expect None when faction is not present in the table
        #[tokio::test]
        async fn returns_none_for_nonexistent_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            let mock_faction = test.eve().with_mock_faction(1);

            let repo = FactionRepository::new(&test.state.db);
            let result = repo.get_by_faction_id(mock_faction.faction_id).await?;

            assert!(result.is_none());

            Ok(())
        }

        /// Expect Error when trying to get faction when required tables have not been created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let faction_id = 1;
            let repo = FactionRepository::new(&test.state.db);
            let result = repo.get_by_faction_id(faction_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
