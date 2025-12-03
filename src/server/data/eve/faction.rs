//! Faction repository for EVE Online faction data management.
//!
//! This module provides the `FactionRepository` for managing faction records from
//! EVE Online's ESI API.

use crate::server::model::db::EveFactionModel;
use chrono::Utc;
use eve_esi::model::universe::Faction;
use migration::OnConflict;
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, Order, QueryFilter, QueryOrder,
    QuerySelect,
};

/// Repository for managing EVE Online faction records in the database.
///
/// Provides operations for upserting faction data from ESI, retrieving faction
/// record IDs, and querying the latest faction update timestamp.
pub struct FactionRepository<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> FactionRepository<'a, C> {
    /// Creates a new instance of FactionRepository.
    ///
    /// Constructs a repository for managing EVE faction records in the database.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    ///
    /// # Returns
    /// - `FactionRepository` - New repository instance
    pub fn new(db: &'a C) -> Self {
        Self { db }
    }

    /// Inserts or updates multiple faction records from ESI data.
    ///
    /// Creates new faction records or updates existing ones based on faction_id.
    /// On conflict, updates all faction fields except created_at.
    ///
    /// # Arguments
    /// - `factions` - Vector of ESI faction data
    ///
    /// # Returns
    /// - `Ok(Vec<EveFaction>)` - The created or updated faction records
    /// - `Err(DbErr)` - Database operation failed
    pub async fn upsert_many(&self, factions: Vec<Faction>) -> Result<Vec<EveFactionModel>, DbErr> {
        let factions = factions
            .into_iter()
            .map(|f| entity::eve_faction::ActiveModel {
                faction_id: ActiveValue::Set(f.faction_id),
                corporation_id: ActiveValue::Set(f.corporation_id),
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

    /// Retrieves internal database record IDs for EVE faction IDs.
    ///
    /// Maps EVE Online faction IDs to their corresponding internal database record IDs.
    /// Returns only entries that exist in the database.
    ///
    /// # Arguments
    /// - `faction_ids` - Slice of EVE faction IDs to look up
    ///
    /// # Returns
    /// - `Ok(Vec<(i32, i64)>)` - List of (record_id, faction_id) tuples for found factions
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_record_ids_by_faction_ids(
        &self,
        faction_ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, DbErr> {
        entity::prelude::EveFaction::find()
            .select_only()
            .column(entity::eve_faction::Column::Id)
            .column(entity::eve_faction::Column::FactionId)
            .filter(entity::eve_faction::Column::FactionId.is_in(faction_ids.iter().copied()))
            .into_tuple::<(i32, i64)>()
            .all(self.db)
            .await
    }

    /// Retrieves the most recently updated faction record.
    ///
    /// Fetches the faction with the latest updated_at timestamp. Useful for determining
    /// when faction data was last refreshed from ESI.
    ///
    /// # Returns
    /// - `Ok(Some(EveFaction))` - The most recently updated faction record
    /// - `Ok(None)` - No factions exist in the database
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_latest(&self) -> Result<Option<EveFactionModel>, DbErr> {
        entity::prelude::EveFaction::find()
            .order_by(entity::eve_faction::Column::UpdatedAt, Order::Desc)
            .one(self.db)
            .await
    }
}

#[cfg(test)]
mod tests {
    use bifrost_test_utils::prelude::*;

    use super::*;

    /// Tests for FactionRepository::upsert_many method.
    mod upsert_many {
        use super::*;

        /// Tests upserting a new faction.
        ///
        /// Verifies that the faction repository successfully inserts a new faction
        /// record into the database.
        ///
        /// Expected: Ok with Vec containing 1 created faction
        #[tokio::test]
        async fn upserts_new_faction() -> Result<(), TestError> {
            let mut test = TestBuilder::new()
                .with_table(entity::prelude::EveFaction)
                .build()
                .await?;
            let mock_faction = test.eve().mock_faction(1);

            let repo = FactionRepository::new(&test.db);
            let result = repo.upsert_many(vec![mock_faction]).await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let created_factions = result.unwrap();
            assert_eq!(created_factions.len(), 1);

            Ok(())
        }

        /// Tests updating an existing faction.
        ///
        /// Verifies that the faction repository updates an existing faction record when
        /// upserting with the same faction ID, preserving created_at and updating
        /// updated_at timestamp.
        ///
        /// Expected: Ok with updated faction, preserved created_at, newer updated_at
        #[tokio::test]
        async fn updates_existing_faction() -> Result<(), TestError> {
            let mut test = TestBuilder::new()
                .with_table(entity::prelude::EveFaction)
                .build()
                .await?;
            let mock_faction = test.eve().mock_faction(1);
            let mock_faction_update = test.eve().mock_faction(1);

            let repo = FactionRepository::new(&test.db);
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

    /// Tests for FactionRepository::get_record_ids_by_faction_ids method.
    mod get_record_ids_by_faction_ids {
        use super::*;

        /// Tests retrieving record IDs for existing factions.
        ///
        /// Verifies that the faction repository correctly maps faction IDs to their
        /// corresponding database record IDs when all requested factions exist.
        ///
        /// Expected: Ok with Vec of (record_id, faction_id) tuples
        #[tokio::test]
        async fn returns_record_ids_for_existing_factions() -> Result<(), TestError> {
            let mut test = TestBuilder::new()
                .with_table(entity::prelude::EveFaction)
                .build()
                .await?;
            let faction_1 = test.eve().insert_mock_faction(1).await?;
            let faction_2 = test.eve().insert_mock_faction(2).await?;
            let faction_3 = test.eve().insert_mock_faction(3).await?;

            let repo = FactionRepository::new(&test.db);
            let faction_ids = vec![
                faction_1.faction_id,
                faction_2.faction_id,
                faction_3.faction_id,
            ];
            let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

            assert!(result.is_ok());
            let record_ids = result.unwrap();
            assert_eq!(record_ids.len(), 3);

            // Verify the mappings are correct
            let mut found_ids = std::collections::HashSet::new();
            for (record_id, faction_id) in record_ids {
                match faction_id {
                    _ if faction_id == faction_1.faction_id => {
                        assert_eq!(record_id, faction_1.id);
                    }
                    _ if faction_id == faction_2.faction_id => {
                        assert_eq!(record_id, faction_2.id);
                    }
                    _ if faction_id == faction_3.faction_id => {
                        assert_eq!(record_id, faction_3.id);
                    }
                    _ => panic!("Unexpected faction_id: {}", faction_id),
                }
                found_ids.insert(faction_id);
            }
            assert_eq!(found_ids.len(), 3);

            Ok(())
        }

        /// Tests retrieving record IDs for nonexistent factions.
        ///
        /// Verifies that the faction repository returns an empty list when attempting
        /// to retrieve record IDs for faction IDs that do not exist in the database.
        ///
        /// Expected: Ok with empty Vec
        #[tokio::test]
        async fn returns_empty_for_nonexistent_factions() -> Result<(), TestError> {
            let test = TestBuilder::new()
                .with_table(entity::prelude::EveFaction)
                .build()
                .await?;

            let repo = FactionRepository::new(&test.db);
            let faction_ids = vec![1, 2, 3];
            let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

            assert!(result.is_ok());
            let record_ids = result.unwrap();
            assert_eq!(record_ids.len(), 0);

            Ok(())
        }

        /// Tests retrieving record IDs with empty input.
        ///
        /// Verifies that the faction repository handles empty input lists gracefully
        /// by returning an empty result without errors.
        ///
        /// Expected: Ok with empty Vec
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test = TestBuilder::new()
                .with_table(entity::prelude::EveFaction)
                .build()
                .await?;

            let repo = FactionRepository::new(&test.db);
            let faction_ids: Vec<i64> = vec![];
            let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

            assert!(result.is_ok());
            let record_ids = result.unwrap();
            assert_eq!(record_ids.len(), 0);

            Ok(())
        }

        /// Tests retrieving record IDs with mixed input.
        ///
        /// Verifies that the faction repository returns partial results when only some
        /// of the requested faction IDs exist, excluding nonexistent IDs from the output.
        ///
        /// Expected: Ok with Vec containing only existing faction mappings
        #[tokio::test]
        async fn returns_partial_results_for_mixed_input() -> Result<(), TestError> {
            let mut test = TestBuilder::new()
                .with_table(entity::prelude::EveFaction)
                .build()
                .await?;
            let faction_1 = test.eve().insert_mock_faction(1).await?;
            let faction_3 = test.eve().insert_mock_faction(3).await?;

            let repo = FactionRepository::new(&test.db);
            let faction_ids = vec![
                faction_1.faction_id,
                999, // Non-existent
                faction_3.faction_id,
                888, // Non-existent
            ];
            let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

            assert!(result.is_ok());
            let record_ids = result.unwrap();
            assert_eq!(record_ids.len(), 2);

            // Verify only existing factions are returned
            for (record_id, faction_id) in record_ids {
                assert!(faction_id == faction_1.faction_id || faction_id == faction_3.faction_id);
                if faction_id == faction_1.faction_id {
                    assert_eq!(record_id, faction_1.id);
                } else if faction_id == faction_3.faction_id {
                    assert_eq!(record_id, faction_3.id);
                }
            }

            Ok(())
        }

        /// Tests error handling when database tables are missing.
        ///
        /// Verifies that the faction repository returns an error when attempting to
        /// retrieve record IDs without the required database tables being created.
        ///
        /// Expected: Err
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = TestBuilder::new().build().await?;

            let repo = FactionRepository::new(&test.db);
            let faction_ids = vec![1, 2, 3];
            let result = repo.get_record_ids_by_faction_ids(&faction_ids).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
