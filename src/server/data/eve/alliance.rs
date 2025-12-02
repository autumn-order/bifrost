use chrono::Utc;
use eve_esi::model::alliance::Alliance;
use migration::OnConflict;
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect,
};

pub struct AllianceRepository<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> AllianceRepository<'a, C> {
    pub fn new(db: &'a C) -> Self {
        Self { db }
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
            .exec_with_returning(self.db)
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
            .all(self.db)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bifrost_test_utils::prelude::*;

    mod upsert_many {
        use super::*;

        /// Expect Ok when upserting new alliances
        #[tokio::test]
        async fn upserts_new_alliances() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id_1, alliance_1) = test.eve().with_mock_alliance(1, None);
            let (alliance_id_2, alliance_2) = test.eve().with_mock_alliance(2, None);

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let result = alliance_repo
                .upsert_many(vec![
                    (alliance_id_1, alliance_1, None),
                    (alliance_id_2, alliance_2, None),
                ])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let created_alliances = result.unwrap();
            assert_eq!(created_alliances.len(), 2);

            Ok(())
        }

        /// Expect Ok & update when trying to upsert existing alliances
        #[tokio::test]
        async fn updates_existing_alliances() -> Result<(), TestError> {
            let mut test =
                test_setup_with_tables!(entity::prelude::EveFaction, entity::prelude::EveAlliance)?;
            let (alliance_id_1, alliance_1) = test.eve().with_mock_alliance(1, None);
            let (alliance_id_2, alliance_2) = test.eve().with_mock_alliance(2, None);
            let (alliance_id_1_update, alliance_1_update) = test.eve().with_mock_alliance(1, None);
            let (alliance_id_2_update, alliance_2_update) = test.eve().with_mock_alliance(2, None);

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let initial = alliance_repo
                .upsert_many(vec![
                    (alliance_id_1, alliance_1, None),
                    (alliance_id_2, alliance_2, None),
                ])
                .await?;

            let initial_entry_1 = initial
                .iter()
                .find(|a| a.alliance_id == alliance_id_1)
                .expect("alliance 1 not found");
            let initial_entry_2 = initial
                .iter()
                .find(|a| a.alliance_id == alliance_id_2)
                .expect("alliance 2 not found");

            let initial_created_at_1 = initial_entry_1.created_at;
            let initial_updated_at_1 = initial_entry_1.updated_at;
            let initial_created_at_2 = initial_entry_2.created_at;
            let initial_updated_at_2 = initial_entry_2.updated_at;

            let latest = alliance_repo
                .upsert_many(vec![
                    (alliance_id_1_update, alliance_1_update, None),
                    (alliance_id_2_update, alliance_2_update, None),
                ])
                .await?;

            let latest_entry_1 = latest
                .iter()
                .find(|a| a.alliance_id == alliance_id_1_update)
                .expect("alliance 1 not found");
            let latest_entry_2 = latest
                .iter()
                .find(|a| a.alliance_id == alliance_id_2_update)
                .expect("alliance 2 not found");

            // created_at should not change and updated_at should increase for both alliances
            assert_eq!(latest_entry_1.created_at, initial_created_at_1);
            assert!(latest_entry_1.updated_at > initial_updated_at_1);
            assert_eq!(latest_entry_2.created_at, initial_created_at_2);
            assert!(latest_entry_2.updated_at > initial_updated_at_2);

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

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let alliance_ids = vec![
                alliance_1.alliance_id,
                alliance_2.alliance_id,
                alliance_3.alliance_id,
            ];
            let result = alliance_repo
                .get_entry_ids_by_alliance_ids(&alliance_ids)
                .await;

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

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let alliance_ids = vec![1, 2, 3];
            let result = alliance_repo
                .get_entry_ids_by_alliance_ids(&alliance_ids)
                .await;

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

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let alliance_ids: Vec<i64> = vec![];
            let result = alliance_repo
                .get_entry_ids_by_alliance_ids(&alliance_ids)
                .await;

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

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let alliance_ids = vec![
                alliance_1.alliance_id,
                999, // Non-existent
                alliance_3.alliance_id,
                888, // Non-existent
            ];
            let result = alliance_repo
                .get_entry_ids_by_alliance_ids(&alliance_ids)
                .await;

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

            let alliance_repo = AllianceRepository::new(&test.state.db);
            let alliance_ids = vec![1, 2, 3];
            let result = alliance_repo
                .get_entry_ids_by_alliance_ids(&alliance_ids)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
