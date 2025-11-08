use chrono::Utc;
use eve_esi::model::character::Character;
use migration::{CaseStatement, Expr, OnConflict};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QuerySelect, TransactionTrait,
};

pub struct CharacterRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> CharacterRepository<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        character_id: i64,
        character: Character,
        corporation_id: i32,
        faction_id: Option<i32>,
    ) -> Result<entity::eve_character::Model, DbErr> {
        let character = entity::eve_character::ActiveModel {
            character_id: ActiveValue::Set(character_id),
            corporation_id: ActiveValue::Set(corporation_id),
            faction_id: ActiveValue::Set(faction_id),
            birthday: ActiveValue::Set(character.birthday.naive_utc()),
            bloodline_id: ActiveValue::Set(character.bloodline_id),
            description: ActiveValue::Set(character.description),
            gender: ActiveValue::Set(character.gender),
            name: ActiveValue::Set(character.name),
            race_id: ActiveValue::Set(character.race_id),
            security_status: ActiveValue::Set(character.security_status),
            title: ActiveValue::Set(character.title),
            created_at: ActiveValue::Set(Utc::now().naive_utc()),
            updated_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        character.insert(self.db).await
    }

    pub async fn upsert(
        &self,
        character_id: i64,
        character: Character,
        corporation_id: i32,
        faction_id: Option<i32>,
    ) -> Result<entity::eve_character::Model, DbErr> {
        Ok(
            entity::prelude::EveCharacter::insert(entity::eve_character::ActiveModel {
                character_id: ActiveValue::Set(character_id),
                corporation_id: ActiveValue::Set(corporation_id),
                faction_id: ActiveValue::Set(faction_id),
                birthday: ActiveValue::Set(character.birthday.naive_utc()),
                bloodline_id: ActiveValue::Set(character.bloodline_id),
                description: ActiveValue::Set(character.description),
                gender: ActiveValue::Set(character.gender),
                name: ActiveValue::Set(character.name),
                race_id: ActiveValue::Set(character.race_id),
                security_status: ActiveValue::Set(character.security_status),
                title: ActiveValue::Set(character.title),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            })
            .on_conflict(
                OnConflict::column(entity::eve_character::Column::CharacterId)
                    .update_columns([
                        entity::eve_character::Column::CorporationId,
                        entity::eve_character::Column::FactionId,
                        entity::eve_character::Column::Birthday,
                        entity::eve_character::Column::BloodlineId,
                        entity::eve_character::Column::Description,
                        entity::eve_character::Column::Gender,
                        entity::eve_character::Column::Name,
                        entity::eve_character::Column::RaceId,
                        entity::eve_character::Column::SecurityStatus,
                        entity::eve_character::Column::Title,
                        entity::eve_character::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(self.db)
            .await?,
        )
    }

    pub async fn upsert_many(
        &self,
        characters: Vec<(i64, Character, i32, Option<i32>)>,
    ) -> Result<Vec<entity::eve_character::Model>, DbErr> {
        let characters =
            characters
                .into_iter()
                .map(|(character_id, character, corporation_id, faction_id)| {
                    entity::eve_character::ActiveModel {
                        character_id: ActiveValue::Set(character_id),
                        corporation_id: ActiveValue::Set(corporation_id),
                        faction_id: ActiveValue::Set(faction_id),
                        birthday: ActiveValue::Set(character.birthday.naive_utc()),
                        bloodline_id: ActiveValue::Set(character.bloodline_id),
                        description: ActiveValue::Set(character.description),
                        gender: ActiveValue::Set(character.gender),
                        name: ActiveValue::Set(character.name),
                        race_id: ActiveValue::Set(character.race_id),
                        security_status: ActiveValue::Set(character.security_status),
                        title: ActiveValue::Set(character.title),
                        created_at: ActiveValue::Set(Utc::now().naive_utc()),
                        updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                        ..Default::default()
                    }
                });

        entity::prelude::EveCharacter::insert_many(characters)
            .on_conflict(
                OnConflict::column(entity::eve_character::Column::CharacterId)
                    .update_columns([
                        entity::eve_character::Column::CorporationId,
                        entity::eve_character::Column::FactionId,
                        entity::eve_character::Column::Birthday,
                        entity::eve_character::Column::BloodlineId,
                        entity::eve_character::Column::Description,
                        entity::eve_character::Column::Gender,
                        entity::eve_character::Column::Name,
                        entity::eve_character::Column::RaceId,
                        entity::eve_character::Column::SecurityStatus,
                        entity::eve_character::Column::Title,
                        entity::eve_character::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(self.db)
            .await
    }

    pub async fn get_by_character_id(
        &self,
        character_id: i64,
    ) -> Result<Option<entity::eve_character::Model>, DbErr> {
        entity::prelude::EveCharacter::find()
            .filter(entity::eve_character::Column::CharacterId.eq(character_id))
            .one(self.db)
            .await
    }

    pub async fn get_entry_ids_by_character_ids(
        &self,
        character_ids: &[i64],
    ) -> Result<Vec<(i32, i64)>, DbErr> {
        entity::prelude::EveCharacter::find()
            .select_only()
            .column(entity::eve_character::Column::Id)
            .column(entity::eve_character::Column::CharacterId)
            .filter(entity::eve_character::Column::CharacterId.is_in(character_ids.iter().copied()))
            .into_tuple::<(i32, i64)>()
            .all(self.db)
            .await
    }

    /// Updates a list of characters to the provided corporation & faction IDs
    ///
    /// # Arguments
    /// - `characters`: Vector of tuples containing the character ID & the corporation ID
    ///   & faction ID to update them to
    ///
    /// # Notes
    /// - Corporation IDs must exist in the eve_corporation table due to foreign key constraint
    /// - Faction IDs must exist in the eve_faction table due to foreign key constraint
    /// - Characters that don't exist will be silently skipped
    pub async fn update_affiliations(
        &self,
        characters: Vec<(i32, i32, Option<i32>)>, // (character_id, corporation_id, faction_id)
    ) -> Result<(), DbErr> {
        if characters.is_empty() {
            return Ok(());
        }

        let txn = self.db.begin().await?;

        const BATCH_SIZE: usize = 100;

        for batch in characters.chunks(BATCH_SIZE) {
            let mut corp_case_stmt = CaseStatement::new();
            let mut faction_case_stmt = CaseStatement::new();
            let character_ids: Vec<i32> = batch.iter().map(|(id, _, _)| *id).collect();

            for (character_id, corporation_id, faction_id) in batch {
                corp_case_stmt = corp_case_stmt.case(
                    entity::eve_character::Column::Id.eq(*character_id),
                    Expr::value(*corporation_id),
                );

                faction_case_stmt = faction_case_stmt.case(
                    entity::eve_character::Column::Id.eq(*character_id),
                    Expr::value(*faction_id),
                );
            }

            entity::prelude::EveCharacter::update_many()
                .col_expr(
                    entity::eve_character::Column::CorporationId,
                    Expr::value(corp_case_stmt),
                )
                .col_expr(
                    entity::eve_character::Column::FactionId,
                    Expr::value(faction_case_stmt),
                )
                .col_expr(
                    entity::eve_character::Column::UpdatedAt,
                    Expr::current_timestamp(),
                )
                .filter(entity::eve_character::Column::Id.is_in(character_ids))
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
    use sea_orm::{DbErr, RuntimeErr};

    use super::*;

    mod create {
        use super::*;

        /// Expect success when creating character with a faction ID set
        #[tokio::test]
        async fn creates_character_with_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;
            let (character_id, character) =
                test.eve()
                    .with_mock_character(1, corporation_model.corporation_id, None, None);

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .create(
                    character_id,
                    character,
                    corporation_model.id,
                    Some(faction_model.id),
                )
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();

            assert_eq!(created.character_id, character_id);
            assert_eq!(created.faction_id, Some(faction_model.id));

            Ok(())
        }

        /// Expect success when creating character entry
        #[tokio::test]
        async fn creates_character_without_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;
            let (character_id, character) =
                test.eve()
                    .with_mock_character(1, corporation_model.corporation_id, None, None);

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .create(character_id, character, corporation_model.id, None)
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.character_id, character_id);
            assert_eq!(created.faction_id, None);

            Ok(())
        }

        /// Expect Error when attempting to create a character without a valid corporation ID set
        #[tokio::test]
        async fn fails_for_invalid_corporation_id() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let (character_id, character) = test.eve().with_mock_character(1, 1, None, None);

            let corporation_id = 1;
            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .create(character_id, character, corporation_id, None)
                .await;

            assert!(result.is_err());
            // Assert error code is 787 indicating a foreign key constraint error
            assert!(matches!(
                result,
                Err(DbErr::Query(RuntimeErr::SqlxError(err))) if err
                    .as_database_error()
                    .and_then(|d| d.code().map(|c| c == "787"))
                    .unwrap_or(false)
            ));

            Ok(())
        }
    }

    mod upsert {
        use super::*;

        /// Expect Ok when upserting a new character with faction
        #[tokio::test]
        async fn creates_new_character_with_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;
            let (character_id, character) =
                test.eve()
                    .with_mock_character(1, corporation_model.corporation_id, None, None);

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .upsert(
                    character_id,
                    character,
                    corporation_model.id,
                    Some(faction_model.id),
                )
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();
            let (character_id, character) =
                test.eve()
                    .with_mock_character(1, corporation_model.corporation_id, None, None);
            assert_eq!(created.character_id, character_id);
            assert_eq!(created.name, character.name);
            assert_eq!(created.faction_id, Some(faction_model.id));

            Ok(())
        }

        /// Expect Ok when upserting a new character without faction
        #[tokio::test]
        async fn creates_new_character_without_faction() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let corporation_model = test.eve().insert_mock_corporation(1, None, None).await?;
            let (character_id, character) =
                test.eve()
                    .with_mock_character(1, corporation_model.corporation_id, None, None);

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .upsert(character_id, character, corporation_model.id, None)
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();
            assert_eq!(created.faction_id, None);

            Ok(())
        }

        /// Expect Ok when upserting an existing character and verify it updates
        #[tokio::test]
        async fn updates_existing_character() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

            // Create updated character data with different values
            let corporation_model = test.eve().insert_mock_corporation(2, None, None).await?;
            let (character_id, mut updated_character) =
                test.eve()
                    .with_mock_character(1, corporation_model.corporation_id, None, None);
            updated_character.name = "Updated Character Name".to_string();
            updated_character.description = Some("Updated description".to_string());
            updated_character.security_status = Some(5.0);

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .upsert(
                    character_id,
                    updated_character,
                    character_model.corporation_id,
                    None,
                )
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            // Verify the ID remains the same (it's an update, not a new insert)
            assert_eq!(upserted.id, character_model.id);
            assert_eq!(upserted.character_id, character_model.character_id);
            assert_eq!(upserted.name, "Updated Character Name");
            assert_eq!(
                upserted.description,
                Some("Updated description".to_string())
            );
            assert_eq!(upserted.security_status, Some(5.0));

            Ok(())
        }

        /// Expect Ok when upserting an existing character with a new corporation ID
        #[tokio::test]
        async fn updates_character_corporation_relationship() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let corporation_model1 = test.eve().insert_mock_corporation(1, None, None).await?;
            let corporation_model2 = test.eve().insert_mock_corporation(2, None, None).await?;
            let character_model = test
                .eve()
                .insert_mock_character(1, corporation_model1.corporation_id, None, None)
                .await?;

            // Update character with new corporation
            let (character_id, character) =
                test.eve()
                    .with_mock_character(1, corporation_model2.corporation_id, None, None);

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .upsert(character_id, character, corporation_model2.id, None)
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
            assert_eq!(upserted.corporation_id, corporation_model2.id);
            assert_ne!(upserted.corporation_id, corporation_model1.id);

            Ok(())
        }

        /// Expect Ok when upserting an existing character with a new faction ID
        #[tokio::test]
        async fn updates_character_faction_relationship() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let faction_model1 = test.eve().insert_mock_faction(1).await?;
            let faction_model2 = test.eve().insert_mock_faction(2).await?;
            let character_model = test
                .eve()
                .insert_mock_character(1, 1, None, Some(faction_model1.faction_id))
                .await?;

            // Update character with new faction
            let (character_id, character) =
                test.eve()
                    .with_mock_character(1, 1, None, Some(faction_model2.faction_id));

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .upsert(
                    character_id,
                    character,
                    character_model.corporation_id,
                    Some(faction_model2.id),
                )
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
            assert_eq!(upserted.faction_id, Some(faction_model2.id));
            assert_ne!(upserted.faction_id, Some(faction_model1.id));

            Ok(())
        }

        /// Expect Ok when upserting removes faction relationship
        #[tokio::test]
        async fn removes_faction_relationship_on_upsert() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let character_model = test
                .eve()
                .insert_mock_character(1, 1, None, Some(faction_model.faction_id))
                .await?;

            assert!(character_model.faction_id.is_some());

            // Update character without faction
            let (character_id, character) = test.eve().with_mock_character(1, 1, None, None);

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .upsert(
                    character_id,
                    character,
                    character_model.corporation_id,
                    None,
                )
                .await;

            assert!(result.is_ok());
            let upserted = result.unwrap();

            assert_eq!(upserted.id, character_model.id);
            assert_eq!(upserted.faction_id, None);

            Ok(())
        }

        /// Expect Error when upserting to a table that doesn't exist
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!()?;
            let (character_id, character) = test.eve().with_mock_character(1, 1, None, None);

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .upsert(character_id, character, 1, None)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod get_by_character_id {
        use super::*;

        /// Expect Some when character is present in database
        #[tokio::test]
        async fn finds_existing_character() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .get_by_character_id(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let character_option = result.unwrap();
            assert!(character_option.is_some());

            Ok(())
        }

        /// Expect None when no character entry is present
        #[tokio::test]
        async fn returns_none_for_nonexistent_character() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let character_id = 1;
            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo.get_by_character_id(character_id).await;

            assert!(result.is_ok());
            let character_option = result.unwrap();
            assert!(character_option.is_none());

            Ok(())
        }

        /// Expect Error when required database tables have not been created
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            // Use setup function that doesn't create required tables, causing a database error
            let test = test_setup_with_tables!()?;

            let character_id = 1;
            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo.get_by_character_id(character_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod update_affiliations {
        use super::*;

        /// Should successfully update a single character's corporation and faction affiliation
        #[tokio::test]
        async fn updates_single_character_affiliation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Create factions and corporations
            let faction1 = test.eve().insert_mock_faction(1).await?;
            let faction2 = test.eve().insert_mock_faction(2).await?;
            let corp1 = test
                .eve()
                .insert_mock_corporation(100, None, Some(faction1.faction_id))
                .await?;
            let corp2 = test
                .eve()
                .insert_mock_corporation(200, None, Some(faction2.faction_id))
                .await?;

            // Create a character initially affiliated with corp1 and faction1
            let (character_id, character) =
                test.eve()
                    .with_mock_character(1, corp1.corporation_id, None, None);
            let character_repo = CharacterRepository::new(&test.state.db);
            let char = character_repo
                .create(character_id, character, corp1.id, Some(faction1.id))
                .await?;

            // Update character to be affiliated with corp2 and faction2
            let result = character_repo
                .update_affiliations(vec![(char.id, corp2.id, Some(faction2.id))])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify the update
            let updated = character_repo
                .get_by_character_id(char.character_id)
                .await?
                .expect("Character should exist");

            assert_eq!(updated.corporation_id, corp2.id);
            assert_eq!(updated.faction_id, Some(faction2.id));

            Ok(())
        }

        /// Should successfully update multiple characters in a single call
        #[tokio::test]
        async fn updates_multiple_characters() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Create factions and corporations
            let faction1 = test.eve().insert_mock_faction(1).await?;
            let faction2 = test.eve().insert_mock_faction(2).await?;
            let faction3 = test.eve().insert_mock_faction(3).await?;
            let corp1 = test.eve().insert_mock_corporation(100, None, None).await?;
            let corp2 = test.eve().insert_mock_corporation(200, None, None).await?;
            let corp3 = test.eve().insert_mock_corporation(300, None, None).await?;

            // Create characters
            let char1 = test
                .eve()
                .insert_mock_character(1, corp1.corporation_id, None, None)
                .await?;

            let char2 = test
                .eve()
                .insert_mock_character(2, corp1.corporation_id, None, None)
                .await?;

            let char3 = test
                .eve()
                .insert_mock_character(3, corp1.corporation_id, None, None)
                .await?;

            // Update multiple characters
            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .update_affiliations(vec![
                    (char1.id, corp1.id, Some(faction1.id)),
                    (char2.id, corp2.id, Some(faction2.id)),
                    (char3.id, corp3.id, Some(faction3.id)),
                ])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify all updates
            let updated1 = character_repo
                .get_by_character_id(char1.character_id)
                .await?
                .expect("Character 1 should exist");
            let updated2 = character_repo
                .get_by_character_id(char2.character_id)
                .await?
                .expect("Character 2 should exist");
            let updated3 = character_repo
                .get_by_character_id(char3.character_id)
                .await?
                .expect("Character 3 should exist");

            assert_eq!(updated1.corporation_id, corp1.id);
            assert_eq!(updated1.faction_id, Some(faction1.id));
            assert_eq!(updated2.corporation_id, corp2.id);
            assert_eq!(updated2.faction_id, Some(faction2.id));
            assert_eq!(updated3.corporation_id, corp3.id);
            assert_eq!(updated3.faction_id, Some(faction3.id));

            Ok(())
        }

        /// Should successfully remove faction affiliation by setting to None
        #[tokio::test]
        async fn removes_faction_affiliation() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Create faction and corporation
            let faction = test.eve().insert_mock_faction(1).await?;
            let corp = test.eve().insert_mock_corporation(100, None, None).await?;

            // Create a character with a faction
            let (character_id, character) =
                test.eve()
                    .with_mock_character(1, corp.corporation_id, None, None);
            let character_repo = CharacterRepository::new(&test.state.db);
            let char = character_repo
                .create(character_id, character, corp.id, Some(faction.id))
                .await?;

            // Remove faction affiliation
            let result = character_repo
                .update_affiliations(vec![(char.id, corp.id, None)])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify the faction was removed
            let updated = character_repo
                .get_by_character_id(char.character_id)
                .await?
                .expect("Character should exist");

            assert_eq!(updated.faction_id, None);

            Ok(())
        }

        /// Should handle batching for large numbers of characters (>100)
        #[tokio::test]
        async fn handles_large_batch_updates() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Create a corporation and faction
            let corp = test.eve().insert_mock_corporation(100, None, None).await?;
            let faction = test.eve().insert_mock_faction(1).await?;

            // Create 250 characters (more than 2x BATCH_SIZE)
            let mut characters = Vec::new();
            for i in 0..250 {
                let char = test
                    .eve()
                    .insert_mock_character(1000 + i, corp.corporation_id, None, None)
                    .await?;

                characters.push((char.id, corp.id, Some(faction.id)));
            }

            // Update all characters
            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo.update_affiliations(characters).await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify a sample of updates
            let updated_first = character_repo
                .get_by_character_id(1000)
                .await?
                .expect("First character should exist");
            let updated_middle = character_repo
                .get_by_character_id(1125)
                .await?
                .expect("Middle character should exist");
            let updated_last = character_repo
                .get_by_character_id(1249)
                .await?
                .expect("Last character should exist");

            assert_eq!(updated_first.faction_id, Some(faction.id));
            assert_eq!(updated_middle.faction_id, Some(faction.id));
            assert_eq!(updated_last.faction_id, Some(faction.id));

            Ok(())
        }

        /// Should handle empty input gracefully
        #[tokio::test]
        async fn handles_empty_input() -> Result<(), TestError> {
            let test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo.update_affiliations(vec![]).await;

            assert!(result.is_ok(), "Should handle empty input gracefully");

            Ok(())
        }

        /// Should update UpdatedAt timestamp when updating affiliations
        #[tokio::test]
        async fn updates_timestamp() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Create corporation and faction
            let corp = test.eve().insert_mock_corporation(100, None, None).await?;
            let faction = test.eve().insert_mock_faction(1).await?;

            // Create a character
            let (character_id, character) =
                test.eve()
                    .with_mock_character(1, corp.corporation_id, None, None);
            let character_repo = CharacterRepository::new(&test.state.db);
            let char = character_repo
                .create(character_id, character, corp.id, None)
                .await?;

            let original_updated_at = char.updated_at;

            // Wait a moment to ensure timestamp difference
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // Update the character
            let result = character_repo
                .update_affiliations(vec![(char.id, corp.id, Some(faction.id))])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify the timestamp was updated
            let updated = character_repo
                .get_by_character_id(char.character_id)
                .await?
                .expect("Character should exist");

            assert!(
                updated.updated_at >= original_updated_at,
                "UpdatedAt should be equal to or newer than original. Original: {:?}, Updated: {:?}",
                original_updated_at,
                updated.updated_at
            );

            Ok(())
        }

        /// Should not affect characters not in the update list
        #[tokio::test]
        async fn does_not_affect_other_characters() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Create factions and corporations
            let faction1 = test.eve().insert_mock_faction(1).await?;
            let faction2 = test.eve().insert_mock_faction(2).await?;
            let corp1 = test.eve().insert_mock_corporation(100, None, None).await?;
            let corp2 = test.eve().insert_mock_corporation(200, None, None).await?;

            // Create characters
            let char1 = test
                .eve()
                .insert_mock_character(1, corp1.corporation_id, None, Some(faction1.faction_id))
                .await?;
            let char2 = test
                .eve()
                .insert_mock_character(2, corp1.corporation_id, None, Some(faction1.faction_id))
                .await?;

            // Update only char1
            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .update_affiliations(vec![(char1.id, corp2.id, Some(faction2.id))])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify char1 was updated
            let updated1 = character_repo
                .get_by_character_id(char1.character_id)
                .await?
                .expect("Character 1 should exist");
            assert_eq!(updated1.corporation_id, corp2.id);
            assert_eq!(updated1.faction_id, Some(faction2.id));

            // Verify char2 was NOT updated
            let updated2 = character_repo
                .get_by_character_id(char2.character_id)
                .await?
                .expect("Character 2 should exist");
            assert_eq!(
                updated2.corporation_id, corp1.id,
                "Character 2 should still have original corporation"
            );
            assert_eq!(
                updated2.faction_id,
                Some(faction1.id),
                "Character 2 should still have original faction"
            );

            Ok(())
        }

        /// Should handle mix of Some and None faction IDs in same batch
        #[tokio::test]
        async fn handles_mixed_faction_assignments() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;

            // Create faction and corporation
            let faction = test.eve().insert_mock_faction(1).await?;
            let corp = test.eve().insert_mock_corporation(100, None, None).await?;

            // Create characters
            let char1 = test
                .eve()
                .insert_mock_character(1, corp.corporation_id, None, None)
                .await?;
            let char2 = test
                .eve()
                .insert_mock_character(2, corp.corporation_id, None, None)
                .await?;
            let char3 = test
                .eve()
                .insert_mock_character(3, corp.corporation_id, None, None)
                .await?;

            // Update with mixed faction IDs
            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo
                .update_affiliations(vec![
                    (char1.id, corp.id, Some(faction.id)),
                    (char2.id, corp.id, None),
                    (char3.id, corp.id, Some(faction.id)),
                ])
                .await;

            assert!(result.is_ok(), "Error: {:?}", result);

            // Verify updates
            let updated1 = character_repo
                .get_by_character_id(char1.character_id)
                .await?
                .expect("Character 1 should exist");
            let updated2 = character_repo
                .get_by_character_id(char2.character_id)
                .await?
                .expect("Character 2 should exist");
            let updated3 = character_repo
                .get_by_character_id(char3.character_id)
                .await?
                .expect("Character 3 should exist");

            assert_eq!(updated1.faction_id, Some(faction.id));
            assert_eq!(updated2.faction_id, None);
            assert_eq!(updated3.faction_id, Some(faction.id));

            Ok(())
        }
    }
}
