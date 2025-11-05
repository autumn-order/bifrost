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

    /// Find faction IDs that are missing from the database
    pub async fn find_missing_ids(&self, faction_ids: &[i64]) -> Result<Vec<i64>, DbErr> {
        if faction_ids.is_empty() {
            return Ok(Vec::new());
        }

        let existing_factions: Vec<i64> = entity::prelude::EveFaction::find()
            .filter(entity::eve_faction::Column::FactionId.is_in(faction_ids.iter().copied()))
            .all(self.db)
            .await?
            .into_iter()
            .map(|f| f.faction_id)
            .collect();

        let missing_ids: Vec<i64> = faction_ids
            .iter()
            .filter(|id| !existing_factions.contains(id))
            .copied()
            .collect();

        Ok(missing_ids)
    }
}

#[cfg(test)]
mod tests {
    use bifrost_test_utils::prelude::*;

    use super::*;

    mod upsert_many {
        use super::*;

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
        use super::*;

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

    mod find_missing_ids {
        use super::*;

        /// Expect empty vector when all factions exist
        #[tokio::test]
        async fn returns_empty_when_all_exist() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            let faction1 = test.eve().insert_mock_faction(1).await?;
            let faction2 = test.eve().insert_mock_faction(2).await?;
            let faction3 = test.eve().insert_mock_faction(3).await?;

            let faction_ids = vec![
                faction1.faction_id,
                faction2.faction_id,
                faction3.faction_id,
            ];

            let repo = FactionRepository::new(&test.state.db);
            let result = repo.find_missing_ids(&faction_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert!(missing.is_empty());

            Ok(())
        }

        /// Expect missing IDs when some factions don't exist
        #[tokio::test]
        async fn returns_missing_ids() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;

            // Only insert factions 1 and 3
            let faction1 = test.eve().insert_mock_faction(1).await?;
            let faction2 = test.eve().with_mock_faction(2);
            let faction3 = test.eve().insert_mock_faction(3).await?;
            let faction4 = test.eve().with_mock_faction(4);
            let faction5 = test.eve().with_mock_faction(5);

            // Query for factions 1, 2, 3, 4, 5 where only 1 and 3 exist
            let faction_ids = vec![
                faction1.faction_id,
                faction2.faction_id,
                faction3.faction_id,
                faction4.faction_id,
                faction5.faction_id,
            ];

            let repo = FactionRepository::new(&test.state.db);
            let result = repo.find_missing_ids(&faction_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert_eq!(missing.len(), 3);
            assert!(missing.contains(&faction2.faction_id));
            assert!(missing.contains(&faction4.faction_id));
            assert!(missing.contains(&faction5.faction_id));
            assert!(!missing.contains(&faction1.faction_id));
            assert!(!missing.contains(&faction3.faction_id));

            Ok(())
        }

        /// Expect all IDs returned when no factions exist
        #[tokio::test]
        async fn returns_all_when_none_exist() -> Result<(), TestError> {
            let test = test_setup_with_tables!(entity::prelude::EveFaction)?;

            let repo = FactionRepository::new(&test.state.db);

            let faction_ids = vec![500001, 500002, 500003];
            let result = repo.find_missing_ids(&faction_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert_eq!(missing.len(), 3);
            assert_eq!(missing, faction_ids);

            Ok(())
        }

        /// Expect empty vector when querying with empty input
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(entity::prelude::EveFaction)?;

            let repo = FactionRepository::new(&test.state.db);

            let faction_ids: Vec<i64> = vec![];
            let result = repo.find_missing_ids(&faction_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert!(missing.is_empty());

            Ok(())
        }

        /// Expect Error when querying table that doesn't exist
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let repo = FactionRepository::new(&test.state.db);

            let faction_ids = vec![500001, 500002];
            let result = repo.find_missing_ids(&faction_ids).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
