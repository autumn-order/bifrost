use chrono::Utc;
use eve_esi::model::character::Character;
use migration::OnConflict;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QuerySelect,
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
}
