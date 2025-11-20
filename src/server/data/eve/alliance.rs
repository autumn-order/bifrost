use chrono::Utc;
use eve_esi::model::alliance::Alliance;
use migration::OnConflict;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QuerySelect,
};

pub struct AllianceRepository {
    db: DatabaseConnection,
}

impl AllianceRepository {
    pub fn new(db: DatabaseConnection) -> Self {
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

        alliance.insert(&self.db).await
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
                    ])
                    .to_owned(),
            )
            .exec_with_returning(&self.db)
            .await?,
        )
    }

    /// Create or update many based upon provided alliance ID, ESI alliance model, and optional faction table entry ID
    pub async fn upsert_many(
        &self,
        alliances: Vec<(i64, Alliance, Option<i32>)>,
    ) -> Result<Vec<entity::eve_alliance::Model>, DbErr> {
        let alliances = alliances
            .into_iter()
            .map(
                |(alliance_id, alliance, faction_id)| entity::eve_alliance::ActiveModel {
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
                },
            );

        entity::prelude::EveAlliance::insert_many(alliances)
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
                    ])
                    .to_owned(),
            )
            .exec_with_returning(&self.db)
            .await
    }

    /// Get an alliance using its EVE Online alliance ID
    pub async fn get_by_alliance_id(
        &self,
        alliance_id: i64,
    ) -> Result<Option<entity::eve_alliance::Model>, DbErr> {
        entity::prelude::EveAlliance::find()
            .filter(entity::eve_alliance::Column::AllianceId.eq(alliance_id))
            .one(&self.db)
            .await
    }

    pub async fn get_entry_ids_by_alliance_ids(
        &self,
        alliance_ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, DbErr> {
        entity::prelude::EveAlliance::find()
            .select_only()
            .column(entity::eve_alliance::Column::Id)
            .column(entity::eve_alliance::Column::AllianceId)
            .filter(entity::eve_alliance::Column::AllianceId.is_in(alliance_ids.iter().copied()))
            .into_tuple::<(i32, i64)>()
            .all(&self.db)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bifrost_test_utils::prelude::*;

    mod create {
        use super::*;

        /// Expect Ok when creating an alliance with a faction ID
        #[tokio::test]
        async fn creates_alliance_with_faction_id() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let faction = test.eve().insert_mock_faction(1).await?;
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let result = repo
                .create(alliance_id, mock_alliance.clone(), Some(faction.id))
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let alliance = result.unwrap();
            assert_eq!(alliance.alliance_id, alliance_id);
            assert_eq!(alliance.name, mock_alliance.name);
            assert_eq!(alliance.faction_id, Some(faction.id));

            Ok(())
        }

        /// Expect Ok when creating an alliance without a faction
        #[tokio::test]
        async fn creates_alliance_without_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let result = repo.create(alliance_id, mock_alliance, None).await;

            assert!(result.is_ok());

            Ok(())
        }
    }

    mod upsert {
        use super::*;

        /// Expect Ok when upserting a new alliance with a faction ID
        #[tokio::test]
        async fn creates_new_alliance_with_faction_id() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let faction = test.eve().insert_mock_faction(1).await?;
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let result = repo
                .upsert(alliance_id, mock_alliance.clone(), Some(faction.id))
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let alliance = result.unwrap();
            assert_eq!(alliance.alliance_id, alliance_id);
            assert_eq!(alliance.name, mock_alliance.name);
            assert_eq!(alliance.faction_id, Some(faction.id));

            Ok(())
        }

        /// Expect Ok when upserting a new alliance without a faction
        #[tokio::test]
        async fn creates_new_alliance_without_faction() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let result = repo.upsert(alliance_id, mock_alliance, None).await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Ok & update when upserting an existing alliance
        #[tokio::test]
        async fn updates_existing_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);
            let (alliance_id_update, mock_alliance_update) = test.eve().with_mock_alliance(1, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let initial = repo.upsert(alliance_id, mock_alliance, None).await?;
            let initial_created_at = initial.created_at;
            let initial_updated_at = initial.updated_at;

            let latest = repo
                .upsert(alliance_id_update, mock_alliance_update, None)
                .await?;

            // created_at should not change and updated_at should increase
            assert_eq!(latest.created_at, initial_created_at);
            assert!(latest.updated_at > initial_updated_at);

            Ok(())
        }

        /// Expect Ok & update faction ID when upserting an existing alliance with a new faction
        #[tokio::test]
        async fn updates_alliance_faction_relationship() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let faction = test.eve().insert_mock_faction(1).await?;
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);
            let (alliance_id_update, mock_alliance_update) = test.eve().with_mock_alliance(1, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let initial = repo.upsert(alliance_id, mock_alliance, None).await?;
            assert!(initial.faction_id.is_none());

            let latest = repo
                .upsert(alliance_id_update, mock_alliance_update, Some(faction.id))
                .await?;

            assert_eq!(latest.faction_id, Some(faction.id));

            Ok(())
        }

        /// Expect Ok & faction ID removed when upserting an existing alliance with None for faction ID
        #[tokio::test]
        async fn removes_faction_relationship_on_upsert() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let faction = test.eve().insert_mock_faction(1).await?;
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);
            let (alliance_id_update, mock_alliance_update) = test.eve().with_mock_alliance(1, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let initial = repo
                .upsert(alliance_id, mock_alliance, Some(faction.id))
                .await?;
            assert_eq!(initial.faction_id, Some(faction.id));

            let latest = repo
                .upsert(alliance_id_update, mock_alliance_update, None)
                .await?;

            assert!(latest.faction_id.is_none());

            Ok(())
        }

        /// Expect Error when trying to upsert an alliance when required tables have not been created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!()?;
            let (alliance_id, mock_alliance) = test.eve().with_mock_alliance(1, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let result = repo.upsert(alliance_id, mock_alliance, None).await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod upsert_many {
        use super::*;

        /// Expect Ok when upserting new alliances
        #[tokio::test]
        async fn upserts_new_alliances() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id_1, mock_alliance_1) = test.eve().with_mock_alliance(1, None);
            let (alliance_id_2, mock_alliance_2) = test.eve().with_mock_alliance(2, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let result = repo
                .upsert_many(vec![
                    (alliance_id_1, mock_alliance_1, None),
                    (alliance_id_2, mock_alliance_2, None),
                ])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let created_alliances = result.unwrap();
            assert_eq!(created_alliances.len(), 2);

            Ok(())
        }

        /// Expect Ok & update when upserting existing alliances
        #[tokio::test]
        async fn updates_existing_alliances() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id_1, mock_alliance_1) = test.eve().with_mock_alliance(1, None);
            let (alliance_id_2, mock_alliance_2) = test.eve().with_mock_alliance(2, None);
            let (alliance_id_1_update, mock_alliance_1_update) =
                test.eve().with_mock_alliance(1, None);
            let (alliance_id_2_update, mock_alliance_2_update) =
                test.eve().with_mock_alliance(2, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let initial = repo
                .upsert_many(vec![
                    (alliance_id_1, mock_alliance_1, None),
                    (alliance_id_2, mock_alliance_2, None),
                ])
                .await?;

            let initial_alliance_1 = initial
                .iter()
                .find(|a| a.alliance_id == alliance_id_1_update)
                .expect("Alliance 1 not found");
            let initial_alliance_2 = initial
                .iter()
                .find(|a| a.alliance_id == alliance_id_2_update)
                .expect("Alliance 2 not found");

            let initial_1_created_at = initial_alliance_1.created_at;
            let initial_1_updated_at = initial_alliance_1.updated_at;
            let initial_2_created_at = initial_alliance_2.created_at;
            let initial_2_updated_at = initial_alliance_2.updated_at;

            let latest = repo
                .upsert_many(vec![
                    (alliance_id_1_update, mock_alliance_1_update, None),
                    (alliance_id_2_update, mock_alliance_2_update, None),
                ])
                .await?;

            let latest_alliance_1 = latest.iter().find(|a| a.alliance_id == 1).unwrap();
            let latest_alliance_2 = latest.iter().find(|a| a.alliance_id == 2).unwrap();

            // created_at should not change and updated_at should increase for both
            assert_eq!(latest_alliance_1.created_at, initial_1_created_at);
            assert!(latest_alliance_1.updated_at > initial_1_updated_at);
            assert_eq!(latest_alliance_2.created_at, initial_2_created_at);
            assert!(latest_alliance_2.updated_at > initial_2_updated_at);

            Ok(())
        }
    }

    mod get_by_alliance_id {
        use super::*;

        /// Expect Some when alliance is present in the table
        #[tokio::test]
        async fn finds_existing_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_id = 1;
            let (mock_alliance_id, mock_alliance) =
                test.eve().with_mock_alliance(alliance_id, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let initial = repo.upsert(mock_alliance_id, mock_alliance, None).await?;
            let result = repo.get_by_alliance_id(alliance_id).await?;

            assert!(result.is_some());
            let alliance = result.unwrap();

            assert_eq!(initial.id, alliance.id);
            assert_eq!(initial.alliance_id, alliance.alliance_id);

            Ok(())
        }

        /// Expect None when alliance is not present in the table
        #[tokio::test]
        async fn returns_none_for_nonexistent_alliance() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id, _mock_alliance) = test.eve().with_mock_alliance(1, None);

            let repo = AllianceRepository::new(test.state.db.clone());
            let result = repo.get_by_alliance_id(alliance_id).await?;

            assert!(result.is_none());

            Ok(())
        }

        /// Expect Error when trying to get alliance when required tables have not been created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let alliance_id = 1;
            let repo = AllianceRepository::new(test.state.db.clone());
            let result = repo.get_by_alliance_id(alliance_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod get_entry_ids_by_alliance_ids {
        use super::*;

        /// Expect Ok with correct mappings when alliances exist in database
        #[tokio::test]
        async fn returns_entry_ids_for_existing_alliances() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance_2 = test.eve().insert_mock_alliance(2, None).await?;
            let alliance_3 = test.eve().insert_mock_alliance(3, None).await?;

            let repo = AllianceRepository::new(test.state.db.clone());
            let alliance_ids = vec![
                alliance_1.alliance_id,
                alliance_2.alliance_id,
                alliance_3.alliance_id,
            ];
            let result = repo.get_entry_ids_by_alliance_ids(&alliance_ids).await;

            assert!(result.is_ok());
            let entry_ids = result.unwrap();
            assert_eq!(entry_ids.len(), 3);

            // Verify the mappings are correct
            let mut found_ids = std::collections::HashSet::new();
            for (entry_id, alliance_id) in entry_ids {
                match alliance_id {
                    _ if alliance_id == alliance_1.alliance_id => {
                        assert_eq!(entry_id, alliance_1.id);
                    }
                    _ if alliance_id == alliance_2.alliance_id => {
                        assert_eq!(entry_id, alliance_2.id);
                    }
                    _ if alliance_id == alliance_3.alliance_id => {
                        assert_eq!(entry_id, alliance_3.id);
                    }
                    _ => panic!("Unexpected alliance_id: {}", alliance_id),
                }
                found_ids.insert(alliance_id);
            }
            assert_eq!(found_ids.len(), 3);

            Ok(())
        }

        /// Expect Ok with empty Vec when no alliances match
        #[tokio::test]
        async fn returns_empty_for_nonexistent_alliances() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let repo = AllianceRepository::new(test.state.db.clone());
            let alliance_ids = vec![1, 2, 3];
            let result = repo.get_entry_ids_by_alliance_ids(&alliance_ids).await;

            assert!(result.is_ok());
            let entry_ids = result.unwrap();
            assert_eq!(entry_ids.len(), 0);

            Ok(())
        }

        /// Expect Ok with empty Vec when input is empty
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;

            let repo = AllianceRepository::new(test.state.db.clone());
            let alliance_ids: Vec<i64> = vec![];
            let result = repo.get_entry_ids_by_alliance_ids(&alliance_ids).await;

            assert!(result.is_ok());
            let entry_ids = result.unwrap();
            assert_eq!(entry_ids.len(), 0);

            Ok(())
        }

        /// Expect Ok with partial results when only some alliances exist
        #[tokio::test]
        async fn returns_partial_results_for_mixed_input() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let alliance_1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance_3 = test.eve().insert_mock_alliance(3, None).await?;

            let repo = AllianceRepository::new(test.state.db.clone());
            let alliance_ids = vec![
                alliance_1.alliance_id,
                999, // Non-existent
                alliance_3.alliance_id,
                888, // Non-existent
            ];
            let result = repo.get_entry_ids_by_alliance_ids(&alliance_ids).await;

            assert!(result.is_ok());
            let entry_ids = result.unwrap();
            assert_eq!(entry_ids.len(), 2);

            // Verify only existing alliances are returned
            for (entry_id, alliance_id) in entry_ids {
                assert!(
                    alliance_id == alliance_1.alliance_id || alliance_id == alliance_3.alliance_id
                );
                if alliance_id == alliance_1.alliance_id {
                    assert_eq!(entry_id, alliance_1.id);
                } else if alliance_id == alliance_3.alliance_id {
                    assert_eq!(entry_id, alliance_3.id);
                }
            }

            Ok(())
        }

        /// Expect Error when required tables haven't been created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let repo = AllianceRepository::new(test.state.db.clone());
            let alliance_ids = vec![1, 2, 3];
            let result = repo.get_entry_ids_by_alliance_ids(&alliance_ids).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
