use chrono::Utc;
use eve_esi::model::alliance::Alliance;
use migration::OnConflict;
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

    /// Create or update an alliance entry using its ESI model
    pub async fn upsert(
        &self,
        alliance_id: i64,
        alliance: Alliance,
        faction_id: Option<i32>,
    ) -> Result<entity::eve_alliance::Model, DbErr> {
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
                job_scheduled_at: ActiveValue::Set(None),
                ..Default::default()
            })
            .on_conflict(
                OnConflict::column(entity::eve_alliance::Column::AllianceId)
                    .update_columns([
                        entity::eve_alliance::Column::FactionId,
                        entity::eve_alliance::Column::CreatorCorporationId,
                        entity::eve_alliance::Column::ExecutorCorporationId,
                        entity::eve_alliance::Column::CreatorId,
                        entity::eve_alliance::Column::DateFounded,
                        entity::eve_alliance::Column::Name,
                        entity::eve_alliance::Column::Ticker,
                        entity::eve_alliance::Column::UpdatedAt,
                        entity::eve_alliance::Column::JobScheduledAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(self.db)
            .await?,
        )
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

    /// Find alliance IDs that don't exist in the database
    pub async fn find_missing_ids(&self, alliance_ids: &[i64]) -> Result<Vec<i64>, DbErr> {
        if alliance_ids.is_empty() {
            return Ok(Vec::new());
        }

        let existing_alliances: Vec<i64> = entity::prelude::EveAlliance::find()
            .filter(entity::eve_alliance::Column::AllianceId.is_in(alliance_ids.iter().copied()))
            .all(self.db)
            .await?
            .into_iter()
            .map(|a| a.alliance_id)
            .collect();

        let missing_ids: Vec<i64> = alliance_ids
            .iter()
            .filter(|id| !existing_alliances.contains(id))
            .copied()
            .collect();

        Ok(missing_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bifrost_test_utils::prelude::*;

    mod create {
        use super::*;

        /// Expect Ok when creating alliance entry with a related faction
        #[tokio::test]
        async fn creates_alliance_with_faction_id() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let (alliance_id, alliance) = test
                .eve()
                .with_mock_alliance(1, Some(faction_model.faction_id));

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let result = alliance_repo
                .create(alliance_id, alliance, Some(faction_model.id))
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();

            let (alliance_id, alliance) = test
                .eve()
                .with_mock_alliance(1, Some(faction_model.faction_id));
            assert_eq!(created.alliance_id, alliance_id);
            assert_eq!(created.name, alliance.name);
            assert_eq!(created.faction_id, Some(faction_model.id));

            Ok(())
        }

        /// Expect Ok when creating alliance entry without faction ID
        #[tokio::test]
        async fn creates_alliance_without_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id, alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let result = alliance_repo.create(alliance_id, alliance, None).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.faction_id, None);

            Ok(())
        }
    }

    mod upsert {
        use super::*;

        /// Expect Ok when upserting a new alliance entry with a related faction
        #[tokio::test]
        async fn creates_new_alliance_with_faction_id() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let (alliance_id, alliance) = test
                .eve()
                .with_mock_alliance(1, Some(faction_model.faction_id));

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let result = alliance_repo
                .upsert(alliance_id, alliance, Some(faction_model.id))
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();

            let (alliance_id, alliance) = test
                .eve()
                .with_mock_alliance(1, Some(faction_model.faction_id));
            assert_eq!(created.alliance_id, alliance_id);
            assert_eq!(created.name, alliance.name);
            assert_eq!(created.faction_id, Some(faction_model.id));

            Ok(())
        }

        /// Expect Ok when upserting a new alliance entry without faction ID
        #[tokio::test]
        async fn creates_new_alliance_without_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id, alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let result = alliance_repo.upsert(alliance_id, alliance, None).await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.faction_id, None);

            Ok(())
        }

        /// Expect Ok when upserting an existing alliance entry and verify it updates
        #[tokio::test]
        async fn updates_existing_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;

            // Create updated alliance data with different values
            let (alliance_id, mut updated_alliance) = test.eve().with_mock_alliance(1, None);
            updated_alliance.name = "Updated Alliance Name".to_string();
            updated_alliance.ticker = "NEW".to_string();

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let result = alliance_repo
                .upsert(alliance_id, updated_alliance, None)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            // Verify the ID remains the same (it's an update, not a new insert)
            assert_eq!(upserted.id, alliance_model.id);
            assert_eq!(upserted.alliance_id, alliance_model.alliance_id);

            Ok(())
        }

        /// Expect Ok when upserting an existing alliance with a new faction ID
        #[tokio::test]
        async fn updates_alliance_faction_relationship() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let faction_model1 = test.eve().insert_mock_faction(1).await?;
            let faction_model2 = test.eve().insert_mock_faction(2).await?;
            let alliance_model = test
                .eve()
                .insert_mock_alliance(1, Some(faction_model1.faction_id))
                .await?;

            // Update alliance with new faction
            let (alliance_id, alliance) = test
                .eve()
                .with_mock_alliance(1, Some(faction_model2.faction_id));

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let result = alliance_repo
                .upsert(alliance_id, alliance, Some(faction_model2.id))
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, alliance_model.id);
            assert_eq!(upserted.faction_id, Some(faction_model2.id));
            assert_ne!(upserted.faction_id, Some(faction_model1.id));

            Ok(())
        }

        /// Expect Ok when upserting removes faction relationship
        #[tokio::test]
        async fn removes_faction_relationship_on_upsert() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let alliance_model = test
                .eve()
                .insert_mock_alliance(1, Some(faction_model.faction_id))
                .await?;

            assert!(alliance_model.faction_id.is_some());

            // Update alliance without faction
            let (alliance_id, alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let result = alliance_repo.upsert(alliance_id, alliance, None).await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, alliance_model.id);
            assert_eq!(upserted.faction_id, None);

            Ok(())
        }

        /// Expect Error when upserting to a table that doesn't exist
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!()?;
            let (alliance_id, alliance) = test.eve().with_mock_alliance(1, None);

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let result = alliance_repo.upsert(alliance_id, alliance, None).await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod get_by_alliance_id {
        use super::*;

        /// Expect Some when getting existing alliance from table
        #[tokio::test]
        async fn finds_existing_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let result = alliance_repo
                .get_by_alliance_id(alliance_model.alliance_id)
                .await;

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
        async fn returns_none_for_nonexistent_alliance() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

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
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let alliance_id = 1;
            let result = alliance_repo.get_by_alliance_id(alliance_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod find_missing_ids {
        use super::*;

        /// Expect empty vector when all alliances exist
        #[tokio::test]
        async fn returns_empty_when_all_exist() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
            let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let alliance_ids = vec![
                alliance1.alliance_id,
                alliance2.alliance_id,
                alliance3.alliance_id,
            ];
            let result = alliance_repo.find_missing_ids(&alliance_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert!(missing.is_empty());

            Ok(())
        }

        /// Expect missing IDs when some alliances don't exist
        #[tokio::test]
        async fn returns_missing_ids() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

            // Query for alliances 1, 2, 3, 4, 5 where only 1 and 3 exist
            let (alliance2_id, _) = test.eve().with_mock_alliance(2, None);
            let (alliance4_id, _) = test.eve().with_mock_alliance(4, None);
            let (alliance5_id, _) = test.eve().with_mock_alliance(5, None);

            let alliance_ids = vec![
                alliance1.alliance_id,
                alliance2_id,
                alliance3.alliance_id,
                alliance4_id,
                alliance5_id,
            ];

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let result = alliance_repo.find_missing_ids(&alliance_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert_eq!(missing.len(), 3);
            assert!(missing.contains(&alliance2_id));
            assert!(missing.contains(&alliance4_id));
            assert!(missing.contains(&alliance5_id));
            assert!(!missing.contains(&alliance1.alliance_id));
            assert!(!missing.contains(&alliance3.alliance_id));

            Ok(())
        }

        /// Expect all IDs returned when no alliances exist
        #[tokio::test]
        async fn returns_all_when_none_exist() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let alliance_ids = vec![100001, 100002, 100003];
            let result = alliance_repo.find_missing_ids(&alliance_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert_eq!(missing.len(), 3);
            assert_eq!(missing, alliance_ids);

            Ok(())
        }

        /// Expect empty vector when querying with empty input
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let alliance_ids: Vec<i64> = vec![];
            let result = alliance_repo.find_missing_ids(&alliance_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert!(missing.is_empty());

            Ok(())
        }

        /// Expect Error when querying table that doesn't exist
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let alliance_repo = AllianceRepository::new(&test.state.db);

            let alliance_ids = vec![100001, 100002];
            let result = alliance_repo.find_missing_ids(&alliance_ids).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
