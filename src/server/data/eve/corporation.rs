use chrono::Utc;
use eve_esi::model::corporation::Corporation;
use migration::{CaseStatement, Expr, OnConflict};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QuerySelect, TransactionTrait,
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

    pub async fn get_entry_ids_by_corporation_ids(
        &self,
        corporation_ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, DbErr> {
        entity::prelude::EveCorporation::find()
            .select_only()
            .column(entity::eve_corporation::Column::Id)
            .column(entity::eve_corporation::Column::CorporationId)
            .filter(
                entity::eve_corporation::Column::CorporationId
                    .is_in(corporation_ids.iter().copied()),
            )
            .into_tuple::<(i32, i64)>()
            .all(self.db)
            .await
    }

    /// Updates a list of corporations to the provided alliance IDs
    ///
    /// # Arguments
    /// - `corporations`: Vector of a tuple containing corporation ID to update and optional alliance ID
    ///
    /// # Notes
    /// - Alliance IDs must exist in the eve_alliance table due to foreign key constraint
    /// - Corporations that don't exist will be silently skipped
    pub async fn update_affiliations(
        &self,
        corporations: Vec<(i32, Option<i32>)>, // (corporation_id, alliance_id)
    ) -> Result<(), DbErr> {
        if corporations.is_empty() {
            return Ok(());
        }

        let txn = self.db.begin().await?;

        const BATCH_SIZE: usize = 100;

        for batch in corporations.chunks(BATCH_SIZE) {
            let mut case_stmt = CaseStatement::new();
            let corporation_ids: Vec<i32> = batch.iter().map(|(id, _)| *id).collect();

            for (corp_id, alliance_id) in batch {
                case_stmt = case_stmt.case(
                    entity::eve_corporation::Column::Id.eq(*corp_id),
                    Expr::value(*alliance_id),
                );
            }

            entity::prelude::EveCorporation::update_many()
                .col_expr(
                    entity::eve_corporation::Column::AllianceId,
                    Expr::value(case_stmt),
                )
                .col_expr(
                    entity::eve_corporation::Column::UpdatedAt,
                    Expr::current_timestamp(),
                )
                .filter(entity::eve_corporation::Column::Id.is_in(corporation_ids))
                .exec(&txn)
                .await?;
        }

        txn.commit().await?;

        Ok(())
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

        /// Expect Ok when upserting mix of new and existing corporations
        #[tokio::test]
        async fn upserts_mixed_new_and_existing_corporations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let (corporation_id_1, corporation_1) = test.eve().with_mock_corporation(1, None, None);
            let (corporation_id_2, corporation_2) = test.eve().with_mock_corporation(2, None, None);
            let (corporation_id_3, corporation_3) = test.eve().with_mock_corporation(3, None, None);
            let (corporation_id_1_update, mut corporation_1_update) =
                test.eve().with_mock_corporation(1, None, None);
            corporation_1_update.name = "Updated Corporation 1".to_string();
            let (corporation_id_2_update, corporation_2_update) =
                test.eve().with_mock_corporation(2, None, None);

            let corporation_repo = CorporationRepository::new(&test.state.db);

            // First, insert corporations 1 and 2
            let initial = corporation_repo
                .upsert_many(vec![
                    (corporation_id_1, corporation_1, None, None),
                    (corporation_id_2, corporation_2, None, None),
                ])
                .await?;

            assert_eq!(initial.len(), 2);
            let initial_corp_1 = initial
                .iter()
                .find(|c| c.corporation_id == corporation_id_1)
                .expect("corporation 1 not found");
            let initial_created_at = initial_corp_1.created_at;

            // Now upsert corporations 1 (update), 2 (update), and 3 (new)
            let result = corporation_repo
                .upsert_many(vec![
                    (corporation_id_1_update, corporation_1_update, None, None),
                    (corporation_id_2_update, corporation_2_update, None, None),
                    (corporation_id_3, corporation_3, None, None),
                ])
                .await?;

            assert_eq!(result.len(), 3);

            let updated_corp_1 = result
                .iter()
                .find(|c| c.corporation_id == corporation_id_1)
                .expect("corporation 1 not found");
            let corp_3 = result
                .iter()
                .find(|c| c.corporation_id == corporation_id_3)
                .expect("corporation 3 not found");

            // Corporation 1 should be updated (same created_at, changed name)
            assert_eq!(updated_corp_1.created_at, initial_created_at);
            assert_eq!(updated_corp_1.name, "Updated Corporation 1");

            // Corporation 3 should be newly created
            assert_eq!(corp_3.corporation_id, corporation_id_3);

            Ok(())
        }

        /// Expect Ok with empty result when upserting empty vector
        #[tokio::test]
        async fn handles_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo.upsert_many(vec![]).await?;

            assert_eq!(result.len(), 0);

            Ok(())
        }

        /// Expect Ok when upserting corporations with various alliance and faction relationships
        #[tokio::test]
        async fn upserts_with_alliance_and_faction_relationships() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let alliance_1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance_2 = test.eve().insert_mock_alliance(2, None).await?;
            let faction_1 = test.eve().insert_mock_faction(1).await?;
            let faction_2 = test.eve().insert_mock_faction(2).await?;

            let (corporation_id_1, corporation_1) = test.eve().with_mock_corporation(
                1,
                Some(alliance_1.alliance_id),
                Some(faction_1.faction_id),
            );
            let (corporation_id_2, corporation_2) =
                test.eve()
                    .with_mock_corporation(2, Some(alliance_2.alliance_id), None);
            let (corporation_id_3, corporation_3) =
                test.eve()
                    .with_mock_corporation(3, None, Some(faction_2.faction_id));
            let (corporation_id_4, corporation_4) = test.eve().with_mock_corporation(4, None, None);

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .upsert_many(vec![
                    (
                        corporation_id_1,
                        corporation_1,
                        Some(alliance_1.id),
                        Some(faction_1.id),
                    ),
                    (corporation_id_2, corporation_2, Some(alliance_2.id), None),
                    (corporation_id_3, corporation_3, None, Some(faction_2.id)),
                    (corporation_id_4, corporation_4, None, None),
                ])
                .await?;

            assert_eq!(result.len(), 4);

            let corp_1 = result
                .iter()
                .find(|c| c.corporation_id == corporation_id_1)
                .unwrap();
            let corp_2 = result
                .iter()
                .find(|c| c.corporation_id == corporation_id_2)
                .unwrap();
            let corp_3 = result
                .iter()
                .find(|c| c.corporation_id == corporation_id_3)
                .unwrap();
            let corp_4 = result
                .iter()
                .find(|c| c.corporation_id == corporation_id_4)
                .unwrap();

            assert_eq!(corp_1.alliance_id, Some(alliance_1.id));
            assert_eq!(corp_1.faction_id, Some(faction_1.id));
            assert_eq!(corp_2.alliance_id, Some(alliance_2.id));
            assert_eq!(corp_2.faction_id, None);
            assert_eq!(corp_3.alliance_id, None);
            assert_eq!(corp_3.faction_id, Some(faction_2.id));
            assert_eq!(corp_4.alliance_id, None);
            assert_eq!(corp_4.faction_id, None);

            Ok(())
        }

        /// Expect Ok when upserting large batch of corporations
        #[tokio::test]
        async fn handles_large_batch() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let mut corporations = Vec::new();
            for i in 1..=100 {
                let (corporation_id, corporation) = test.eve().with_mock_corporation(i, None, None);
                corporations.push((corporation_id, corporation, None, None));
            }

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo.upsert_many(corporations).await?;

            assert_eq!(result.len(), 100);

            // Verify all corporation IDs are present
            for i in 1..=100 {
                assert!(result.iter().any(|c| c.corporation_id == i));
            }

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

    mod get_entry_ids_by_corporation_ids {
        use super::*;

        /// Expect Ok with correct mappings when corporations exist in database
        #[tokio::test]
        async fn returns_entry_ids_for_existing_corporations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_1 = test.eve().insert_mock_corporation(1, None, None).await?;
            let corporation_2 = test.eve().insert_mock_corporation(2, None, None).await?;
            let corporation_3 = test.eve().insert_mock_corporation(3, None, None).await?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let corporation_ids = vec![
                corporation_1.corporation_id,
                corporation_2.corporation_id,
                corporation_3.corporation_id,
            ];
            let result = corporation_repo
                .get_entry_ids_by_corporation_ids(&corporation_ids)
                .await;

            assert!(result.is_ok());
            let entry_ids = result.unwrap();
            assert_eq!(entry_ids.len(), 3);

            // Verify the mappings are correct
            let mut found_ids = std::collections::HashSet::new();
            for (entry_id, corporation_id) in entry_ids {
                match corporation_id {
                    _ if corporation_id == corporation_1.corporation_id => {
                        assert_eq!(entry_id, corporation_1.id);
                    }
                    _ if corporation_id == corporation_2.corporation_id => {
                        assert_eq!(entry_id, corporation_2.id);
                    }
                    _ if corporation_id == corporation_3.corporation_id => {
                        assert_eq!(entry_id, corporation_3.id);
                    }
                    _ => panic!("Unexpected corporation_id: {}", corporation_id),
                }
                found_ids.insert(corporation_id);
            }
            assert_eq!(found_ids.len(), 3);

            Ok(())
        }

        /// Expect Ok with empty Vec when no corporations match
        #[tokio::test]
        async fn returns_empty_for_nonexistent_corporations() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let corporation_ids = vec![1, 2, 3];
            let result = corporation_repo
                .get_entry_ids_by_corporation_ids(&corporation_ids)
                .await;

            assert!(result.is_ok());
            let entry_ids = result.unwrap();
            assert_eq!(entry_ids.len(), 0);

            Ok(())
        }

        /// Expect Ok with empty Vec when input is empty
        #[tokio::test]
        async fn returns_empty_for_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let corporation_ids: Vec<i64> = vec![];
            let result = corporation_repo
                .get_entry_ids_by_corporation_ids(&corporation_ids)
                .await;

            assert!(result.is_ok());
            let entry_ids = result.unwrap();
            assert_eq!(entry_ids.len(), 0);

            Ok(())
        }

        /// Expect Ok with partial results when only some corporations exist
        #[tokio::test]
        async fn returns_partial_results_for_mixed_input() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;
            let corporation_1 = test.eve().insert_mock_corporation(1, None, None).await?;
            let corporation_3 = test.eve().insert_mock_corporation(3, None, None).await?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let corporation_ids = vec![
                corporation_1.corporation_id,
                999, // Non-existent
                corporation_3.corporation_id,
                888, // Non-existent
            ];
            let result = corporation_repo
                .get_entry_ids_by_corporation_ids(&corporation_ids)
                .await;

            assert!(result.is_ok());
            let entry_ids = result.unwrap();
            assert_eq!(entry_ids.len(), 2);

            // Verify only existing corporations are returned
            for (entry_id, corporation_id) in entry_ids {
                assert!(
                    corporation_id == corporation_1.corporation_id
                        || corporation_id == corporation_3.corporation_id
                );
                if corporation_id == corporation_1.corporation_id {
                    assert_eq!(entry_id, corporation_1.id);
                } else if corporation_id == corporation_3.corporation_id {
                    assert_eq!(entry_id, corporation_3.id);
                }
            }

            Ok(())
        }

        /// Expect Error when required tables haven't been created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let corporation_ids = vec![1, 2, 3];
            let result = corporation_repo
                .get_entry_ids_by_corporation_ids(&corporation_ids)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod update_affiliations {
        use super::*;

        /// Should successfully update a single corporation's alliance affiliation
        #[tokio::test]
        async fn updates_single_corporation_alliance() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Create two alliances and a corporation initially affiliated with the first
            let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
            let corp = test
                .eve()
                .insert_mock_corporation(100, Some(alliance1.alliance_id), None)
                .await?;

            // Update corporation to be affiliated with the second alliance
            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .update_affiliations(vec![(corp.id, Some(alliance2.id))])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify the update
            let updated = corporation_repo
                .get_by_corporation_id(corp.corporation_id)
                .await?
                .expect("Corporation should exist");

            assert_eq!(updated.alliance_id, Some(alliance2.id));

            Ok(())
        }

        /// Should successfully update multiple corporations in a single call
        #[tokio::test]
        async fn updates_multiple_corporations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Create alliances
            let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(2, None).await?;
            let alliance3 = test.eve().insert_mock_alliance(3, None).await?;

            // Create corporations
            let corp1 = test.eve().insert_mock_corporation(1, None, None).await?;
            let corp2 = test
                .eve()
                .insert_mock_corporation(2, Some(alliance1.alliance_id), None)
                .await?;
            let corp3 = test.eve().insert_mock_corporation(3, None, None).await?;

            // Update multiple corporations
            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .update_affiliations(vec![
                    (corp1.id, Some(alliance1.id)),
                    (corp2.id, Some(alliance2.id)),
                    (corp3.id, Some(alliance3.id)),
                ])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify all updates
            let updated1 = corporation_repo
                .get_by_corporation_id(corp1.corporation_id)
                .await?
                .expect("Corporation 1 should exist");
            let updated2 = corporation_repo
                .get_by_corporation_id(corp2.corporation_id)
                .await?
                .expect("Corporation 2 should exist");
            let updated3 = corporation_repo
                .get_by_corporation_id(corp3.corporation_id)
                .await?
                .expect("Corporation 3 should exist");

            assert_eq!(updated1.alliance_id, Some(alliance1.id));
            assert_eq!(updated2.alliance_id, Some(alliance2.id));
            assert_eq!(updated3.alliance_id, Some(alliance3.id));

            Ok(())
        }

        /// Should successfully remove alliance affiliation by setting to None
        #[tokio::test]
        async fn removes_alliance_affiliation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Create alliance and corporation with that alliance
            let alliance = test.eve().insert_mock_alliance(1, None).await?;
            let corp = test
                .eve()
                .insert_mock_corporation(100, Some(alliance.alliance_id), None)
                .await?;

            // Remove alliance affiliation
            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .update_affiliations(vec![(corp.id, None)])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify the alliance was removed
            let updated = corporation_repo
                .get_by_corporation_id(corp.corporation_id)
                .await?
                .expect("Corporation should exist");

            assert_eq!(updated.alliance_id, None);

            Ok(())
        }

        /// Should handle batching for large numbers of corporations (>100)
        #[tokio::test]
        async fn handles_large_batch_updates() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Create an alliance
            let alliance = test.eve().insert_mock_alliance(1, None).await?;

            // Create 250 corporations (more than 2x BATCH_SIZE)
            let mut corporations = Vec::new();
            for i in 0..250 {
                let corp = test
                    .eve()
                    .insert_mock_corporation(100 + i, None, None)
                    .await?;
                corporations.push((corp.id, Some(alliance.id)));
            }

            // Update all corporations
            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo.update_affiliations(corporations).await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify a sample of updates
            let updated_first = corporation_repo
                .get_by_corporation_id(100)
                .await?
                .expect("First corporation should exist");
            let updated_middle = corporation_repo
                .get_by_corporation_id(225)
                .await?
                .expect("Middle corporation should exist");
            let updated_last = corporation_repo
                .get_by_corporation_id(349)
                .await?
                .expect("Last corporation should exist");

            assert_eq!(updated_first.alliance_id, Some(alliance.id));
            assert_eq!(updated_middle.alliance_id, Some(alliance.id));
            assert_eq!(updated_last.alliance_id, Some(alliance.id));

            Ok(())
        }

        /// Should handle empty input gracefully
        #[tokio::test]
        async fn handles_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo.update_affiliations(vec![]).await;

            assert!(result.is_ok(), "Should handle empty input gracefully");

            Ok(())
        }

        /// Should update UpdatedAt timestamp when updating affiliations
        #[tokio::test]
        async fn updates_timestamp() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Create alliance and corporation
            let alliance = test.eve().insert_mock_alliance(1, None).await?;
            let corp = test.eve().insert_mock_corporation(100, None, None).await?;

            let original_updated_at = corp.updated_at;

            // Wait a moment to ensure timestamp difference
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // Update the corporation
            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .update_affiliations(vec![(corp.id, Some(alliance.id))])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify the timestamp was updated
            let updated = corporation_repo
                .get_by_corporation_id(corp.corporation_id)
                .await?
                .expect("Corporation should exist");

            assert!(
                updated.updated_at >= original_updated_at,
                "UpdatedAt should be equal to or newer than original. Original: {:?}, Updated: {:?}",
                original_updated_at,
                updated.updated_at
            );

            Ok(())
        }

        /// Should not affect corporations not in the update list
        #[tokio::test]
        async fn does_not_affect_other_corporations() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Create alliances
            let alliance1 = test.eve().insert_mock_alliance(1, None).await?;
            let alliance2 = test.eve().insert_mock_alliance(2, None).await?;

            // Create corporations
            let corp1 = test
                .eve()
                .insert_mock_corporation(100, Some(alliance1.alliance_id), None)
                .await?;
            let corp2 = test
                .eve()
                .insert_mock_corporation(200, Some(alliance1.alliance_id), None)
                .await?;

            // Update only corp1
            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .update_affiliations(vec![(corp1.id, Some(alliance2.id))])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify corp1 was updated
            let updated1 = corporation_repo
                .get_by_corporation_id(corp1.corporation_id)
                .await?
                .expect("Corporation 1 should exist");
            assert_eq!(updated1.alliance_id, Some(alliance2.id));

            // Verify corp2 was NOT updated
            let updated2 = corporation_repo
                .get_by_corporation_id(corp2.corporation_id)
                .await?
                .expect("Corporation 2 should exist");
            assert_eq!(
                updated2.alliance_id,
                Some(alliance1.id),
                "Corporation 2 should still have original alliance"
            );

            Ok(())
        }

        /// Should handle mix of Some and None alliance IDs in same batch
        #[tokio::test]
        async fn handles_mixed_alliance_assignments() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation
            )?;

            // Create alliance
            let alliance = test.eve().insert_mock_alliance(1, None).await?;

            // Create corporations
            let corp1 = test.eve().insert_mock_corporation(100, None, None).await?;
            let corp2 = test
                .eve()
                .insert_mock_corporation(200, Some(alliance.alliance_id), None)
                .await?;
            let corp3 = test.eve().insert_mock_corporation(300, None, None).await?;

            // Update with mixed alliance IDs
            let corporation_repo = CorporationRepository::new(&test.state.db);
            let result = corporation_repo
                .update_affiliations(vec![
                    (corp1.id, Some(alliance.id)),
                    (corp2.id, None),
                    (corp3.id, Some(alliance.id)),
                ])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify updates
            let updated1 = corporation_repo
                .get_by_corporation_id(corp1.corporation_id)
                .await?
                .expect("Corporation 1 should exist");
            let updated2 = corporation_repo
                .get_by_corporation_id(corp2.corporation_id)
                .await?
                .expect("Corporation 2 should exist");
            let updated3 = corporation_repo
                .get_by_corporation_id(corp3.corporation_id)
                .await?
                .expect("Corporation 3 should exist");

            assert_eq!(updated1.alliance_id, Some(alliance.id));
            assert_eq!(updated2.alliance_id, None);
            assert_eq!(updated3.alliance_id, Some(alliance.id));

            Ok(())
        }
    }
}
