use chrono::Utc;
use eve_esi::model::corporation::Corporation;
use migration::OnConflict;
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

    /// Create or update a corporation entry using its ESI model
    pub async fn upsert(
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

        Ok(
            entity::prelude::EveCorporation::insert(entity::eve_corporation::ActiveModel {
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
                job_scheduled_at: ActiveValue::Set(None),
                ..Default::default()
            })
            .on_conflict(
                OnConflict::column(entity::eve_corporation::Column::CorporationId)
                    .update_columns([
                        entity::eve_corporation::Column::AllianceId,
                        entity::eve_corporation::Column::FactionId,
                        entity::eve_corporation::Column::CeoId,
                        entity::eve_corporation::Column::CreatorId,
                        entity::eve_corporation::Column::DateFounded,
                        entity::eve_corporation::Column::Description,
                        entity::eve_corporation::Column::HomeStationId,
                        entity::eve_corporation::Column::MemberCount,
                        entity::eve_corporation::Column::Name,
                        entity::eve_corporation::Column::Shares,
                        entity::eve_corporation::Column::TaxRate,
                        entity::eve_corporation::Column::Ticker,
                        entity::eve_corporation::Column::Url,
                        entity::eve_corporation::Column::WarEligible,
                        entity::eve_corporation::Column::UpdatedAt,
                        entity::eve_corporation::Column::JobScheduledAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(self.db)
            .await?,
        )
    }

    pub async fn upsert_many(
        &self,
        corporations: Vec<(i64, Corporation, Option<i32>, Option<i32>)>,
    ) -> Result<Vec<entity::eve_corporation::Model>, DbErr> {
        let corporations = corporations.into_iter().map(
            |(corporation_id, corporation, alliance_id, faction_id)| {
                let date_founded = match corporation.date_founded {
                    Some(date) => Some(date.naive_utc()),
                    None => None,
                };

                entity::eve_corporation::ActiveModel {
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
                    job_scheduled_at: ActiveValue::Set(None),
                    ..Default::default()
                }
            },
        );

        entity::prelude::EveCorporation::insert_many(corporations)
            .on_conflict(
                OnConflict::column(entity::eve_corporation::Column::CorporationId)
                    .update_columns([
                        entity::eve_corporation::Column::AllianceId,
                        entity::eve_corporation::Column::FactionId,
                        entity::eve_corporation::Column::CeoId,
                        entity::eve_corporation::Column::CreatorId,
                        entity::eve_corporation::Column::DateFounded,
                        entity::eve_corporation::Column::Description,
                        entity::eve_corporation::Column::HomeStationId,
                        entity::eve_corporation::Column::MemberCount,
                        entity::eve_corporation::Column::Name,
                        entity::eve_corporation::Column::Shares,
                        entity::eve_corporation::Column::TaxRate,
                        entity::eve_corporation::Column::Ticker,
                        entity::eve_corporation::Column::Url,
                        entity::eve_corporation::Column::WarEligible,
                        entity::eve_corporation::Column::UpdatedAt,
                        entity::eve_corporation::Column::JobScheduledAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(self.db)
            .await
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

    /// Find corporation IDs that don't exist in the database
    pub async fn find_missing_ids(&self, corporation_ids: &[i64]) -> Result<Vec<i64>, DbErr> {
        if corporation_ids.is_empty() {
            return Ok(Vec::new());
        }

        let existing_corporations: Vec<i64> = entity::prelude::EveCorporation::find()
            .filter(
                entity::eve_corporation::Column::CorporationId
                    .is_in(corporation_ids.iter().copied()),
            )
            .all(self.db)
            .await?
            .into_iter()
            .map(|c| c.corporation_id)
            .collect();

        let missing_ids: Vec<i64> = corporation_ids
            .iter()
            .filter(|id| !existing_corporations.contains(id))
            .copied()
            .collect();

        Ok(missing_ids)
    }
}

#[cfg(test)]
mod tests {
    use bifrost_test_utils::prelude::*;

    use super::*;

    mod create {
        use super::*;

        // Expect Ok when inserting a corporation with both an alliance & faction ID
        #[tokio::test]
        async fn creates_corporation_with_alliance_and_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
            let (corporation_id, corporation) = test.eve().with_mock_corporation(
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
            let (corporation_id, corporation) = test.eve().with_mock_corporation(
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
        async fn creates_corporation_with_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let (corporation_id, corporation) =
                test.eve()
                    .with_mock_corporation(1, None, Some(faction_model.faction_id));

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
        async fn creates_corporation_without_alliance_or_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let (corporation_id, corporation) = test.eve().with_mock_corporation(1, None, None);

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

    mod upsert {
        use super::*;

        /// Expect Ok when upserting a new corporation with both alliance and faction
        #[tokio::test]
        async fn creates_new_corporation_with_alliance_and_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
            let (corporation_id, corporation) = test.eve().with_mock_corporation(
                1,
                Some(alliance_model.alliance_id),
                Some(faction_model.faction_id),
            );

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert(
                    corporation_id,
                    corporation,
                    Some(alliance_model.id),
                    Some(faction_model.id),
                )
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();
            let (corporation_id, corporation) = test.eve().with_mock_corporation(
                1,
                Some(alliance_model.alliance_id),
                Some(faction_model.faction_id),
            );
            assert_eq!(created.corporation_id, corporation_id);
            assert_eq!(created.name, corporation.name);
            assert_eq!(created.alliance_id, Some(alliance_model.id));
            assert_eq!(created.faction_id, Some(faction_model.id));

            Ok(())
        }

        /// Expect Ok when upserting a new corporation with only faction
        #[tokio::test]
        async fn creates_new_corporation_with_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let (corporation_id, corporation) =
                test.eve()
                    .with_mock_corporation(1, None, Some(faction_model.faction_id));

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert(corporation_id, corporation, None, Some(faction_model.id))
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.alliance_id, None);
            assert_eq!(created.faction_id, Some(faction_model.id));

            Ok(())
        }

        /// Expect Ok when upserting a new corporation without alliance or faction
        #[tokio::test]
        async fn creates_new_corporation_without_alliance_or_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let (corporation_id, corporation) = test.eve().with_mock_corporation(1, None, None);

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert(corporation_id, corporation, None, None)
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.alliance_id, None);
            assert_eq!(created.faction_id, None);

            Ok(())
        }

        /// Expect Ok when upserting an existing corporation and verify it updates
        #[tokio::test]
        async fn updates_existing_corporation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

            // Create updated corporation data with different values
            let (corporation_id, mut updated_corporation) =
                test.eve().with_mock_corporation(1, None, None);
            updated_corporation.name = "Updated Corporation Name".to_string();
            updated_corporation.ticker = "NEW".to_string();
            updated_corporation.member_count = 9999;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert(corporation_id, updated_corporation, None, None)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            // Verify the ID remains the same (it's an update, not a new insert)
            assert_eq!(upserted.id, corporation_model.id);
            assert_eq!(upserted.corporation_id, corporation_model.corporation_id);
            assert_eq!(upserted.name, "Updated Corporation Name");
            assert_eq!(upserted.ticker, "NEW");
            assert_eq!(upserted.member_count, 9999);
            assert_eq!(upserted.job_scheduled_at, None);

            Ok(())
        }

        /// Expect Ok when upserting an existing corporation with a new alliance ID
        #[tokio::test]
        async fn updates_corporation_alliance_relationship() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let alliance_model1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance_model2 = test.eve().insert_mock_alliance(2, None).await?;
            let corporation_model = test
                .eve()
                .insert_mock_corporation(1, Some(alliance_model1.alliance_id), None)
                .await?;

            // Update corporation with new alliance
            let (corporation_id, corporation) =
                test.eve()
                    .with_mock_corporation(1, Some(alliance_model2.alliance_id), None);

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert(corporation_id, corporation, Some(alliance_model2.id), None)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_eq!(upserted.alliance_id, Some(alliance_model2.id));
            assert_ne!(upserted.alliance_id, Some(alliance_model1.id));

            Ok(())
        }

        /// Expect Ok when upserting an existing corporation with a new faction ID
        #[tokio::test]
        async fn updates_corporation_faction_relationship() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let faction_model1 = test.eve().insert_mock_faction(1).await?;
            let faction_model2 = test.eve().insert_mock_faction(2).await?;
            let corporation_model = test
                .eve()
                .insert_mock_corporation(1, None, Some(faction_model1.faction_id))
                .await?;

            // Update corporation with new faction
            let (corporation_id, corporation) =
                test.eve()
                    .with_mock_corporation(1, None, Some(faction_model2.faction_id));

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert(corporation_id, corporation, None, Some(faction_model2.id))
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_eq!(upserted.faction_id, Some(faction_model2.id));
            assert_ne!(upserted.faction_id, Some(faction_model1.id));

            Ok(())
        }

        /// Expect Ok when upserting removes alliance relationship
        #[tokio::test]
        async fn removes_alliance_relationship_on_upsert() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
            let corporation_model = test
                .eve()
                .insert_mock_corporation(1, Some(alliance_model.alliance_id), None)
                .await?;

            assert!(corporation_model.alliance_id.is_some());

            // Update corporation without alliance
            let (corporation_id, corporation) = test.eve().with_mock_corporation(1, None, None);

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert(corporation_id, corporation, None, None)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_eq!(upserted.alliance_id, None);

            Ok(())
        }

        /// Expect Ok when upserting removes faction relationship
        #[tokio::test]
        async fn removes_faction_relationship_on_upsert() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let corporation_model = test
                .eve()
                .insert_mock_corporation(1, None, Some(faction_model.faction_id))
                .await?;

            assert!(corporation_model.faction_id.is_some());

            // Update corporation without faction
            let (corporation_id, corporation) = test.eve().with_mock_corporation(1, None, None);

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert(corporation_id, corporation, None, None)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, corporation_model.id);
            assert_eq!(upserted.faction_id, None);

            Ok(())
        }

        /// Expect Error when upserting to a table that doesn't exist
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!()?;
            let (corporation_id, corporation) = test.eve().with_mock_corporation(1, None, None);

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert(corporation_id, corporation, None, None)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod upsert_many {
        use super::*;

        /// Expect Ok when upserting new corporations
        #[tokio::test]
        async fn upserts_new_corporations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let (corporation_id_1, corporation_1) = test.eve().with_mock_corporation(1, None, None);
            let (corporation_id_2, corporation_2) = test.eve().with_mock_corporation(2, None, None);

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert_many(vec![
                    (corporation_id_1, corporation_1, None, None),
                    (corporation_id_2, corporation_2, None, None),
                ])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);
            let created_corporations = result.unwrap();
            assert_eq!(created_corporations.len(), 2);

            Ok(())
        }

        /// Expect Ok & update when trying to upsert existing corporations
        #[tokio::test]
        async fn updates_existing_corporations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let (corporation_id_1, corporation_1) = test.eve().with_mock_corporation(1, None, None);
            let (corporation_id_2, corporation_2) = test.eve().with_mock_corporation(2, None, None);
            let (corporation_id_1_update, corporation_1_update) =
                test.eve().with_mock_corporation(1, None, None);
            let (corporation_id_2_update, corporation_2_update) =
                test.eve().with_mock_corporation(2, None, None);

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let initial = corporation_repo
                .upsert_many(vec![
                    (corporation_id_1, corporation_1, None, None),
                    (corporation_id_2, corporation_2, None, None),
                ])
                .await?;

            let initial_entry_1 = initial
                .iter()
                .find(|c| c.corporation_id == corporation_id_1)
                .expect("corporation 1 not found");
            let initial_entry_2 = initial
                .iter()
                .find(|c| c.corporation_id == corporation_id_2)
                .expect("corporation 2 not found");

            let initial_created_at_1 = initial_entry_1.created_at;
            let initial_updated_at_1 = initial_entry_1.updated_at;
            let initial_created_at_2 = initial_entry_2.created_at;
            let initial_updated_at_2 = initial_entry_2.updated_at;

            let latest = corporation_repo
                .upsert_many(vec![
                    (corporation_id_1_update, corporation_1_update, None, None),
                    (corporation_id_2_update, corporation_2_update, None, None),
                ])
                .await?;

            let latest_entry_1 = latest
                .iter()
                .find(|c| c.corporation_id == corporation_id_1_update)
                .expect("corporation 1 not found");
            let latest_entry_2 = latest
                .iter()
                .find(|c| c.corporation_id == corporation_id_2_update)
                .expect("corporation 2 not found");

            // created_at should not change and updated_at should increase for both corporations
            assert_eq!(latest_entry_1.created_at, initial_created_at_1);
            assert!(latest_entry_1.updated_at > initial_updated_at_1);
            assert_eq!(latest_entry_2.created_at, initial_created_at_2);
            assert!(latest_entry_2.updated_at > initial_updated_at_2);

            Ok(())
        }
    }

    mod get_by_corporation_id {
        use super::*;

        /// Expect Some when getting corporation present in table
        #[tokio::test]
        async fn finds_existing_corporation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .get_by_corporation_id(corporation_model.corporation_id)
                .await;

            assert!(result.is_ok());
            let corporation_option = result.unwrap();
            assert!(corporation_option.is_some());

            Ok(())
        }

        /// Expect None when getting corporation not present in table
        #[tokio::test]
        async fn returns_none_for_nonexistent_corporation() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
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
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let corporation_id = 1;
            let result = corporation_repo.get_by_corporation_id(corporation_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod find_missing_ids {
        use super::*;

        /// Expect empty vector when all corporations exist
        #[tokio::test]
        async fn returns_empty_when_all_exist() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corp1 = test.eve().insert_mock_corporation(1, None, None).await?;
            let corp2 = test.eve().insert_mock_corporation(2, None, None).await?;
            let corp3 = test.eve().insert_mock_corporation(3, None, None).await?;

            let corporation_repo = CorporationRepository::new(&test.state.db);

            let corporation_ids = vec![
                corp1.corporation_id,
                corp2.corporation_id,
                corp3.corporation_id,
            ];
            let result = corporation_repo.find_missing_ids(&corporation_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert!(missing.is_empty());

            Ok(())
        }

        /// Expect missing IDs when some corporations don't exist
        #[tokio::test]
        async fn returns_missing_ids() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corp1 = test.eve().insert_mock_corporation(1, None, None).await?;
            let corp3 = test.eve().insert_mock_corporation(3, None, None).await?;

            // Query for corporations 1, 2, 3, 4, 5 where only 1 and 3 exist
            let (corp2_id, _) = test.eve().with_mock_corporation(2, None, None);
            let (corp4_id, _) = test.eve().with_mock_corporation(4, None, None);
            let (corp5_id, _) = test.eve().with_mock_corporation(5, None, None);

            let corporation_ids = vec![
                corp1.corporation_id,
                corp2_id,
                corp3.corporation_id,
                corp4_id,
                corp5_id,
            ];

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo.find_missing_ids(&corporation_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert_eq!(missing.len(), 3);
            assert!(missing.contains(&corp2_id));
            assert!(missing.contains(&corp4_id));
            assert!(missing.contains(&corp5_id));
            assert!(!missing.contains(&corp1.corporation_id));
            assert!(!missing.contains(&corp3.corporation_id));

            Ok(())
        }

        /// Expect all IDs returned when no corporations exist
        #[tokio::test]
        async fn returns_all_when_none_exist() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_repo = CorporationRepository::new(&test.state.db);

            let corporation_ids = vec![1000001, 1000002, 1000003];
            let result = corporation_repo.find_missing_ids(&corporation_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert_eq!(missing.len(), 3);
            assert_eq!(missing, corporation_ids);

            Ok(())
        }

        /// Expect empty vector when querying with empty input
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_repo = CorporationRepository::new(&test.state.db);

            let corporation_ids: Vec<i64> = vec![];
            let result = corporation_repo.find_missing_ids(&corporation_ids).await;

            assert!(result.is_ok());
            let missing = result.unwrap();
            assert!(missing.is_empty());

            Ok(())
        }

        /// Expect Error when querying table that doesn't exist
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let corporation_repo = CorporationRepository::new(&test.state.db);

            let corporation_ids = vec![1000001, 1000002];
            let result = corporation_repo.find_missing_ids(&corporation_ids).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
