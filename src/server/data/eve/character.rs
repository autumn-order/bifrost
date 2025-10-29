use chrono::Utc;
use eve_esi::model::character::Character;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
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

    pub async fn get_by_character_id(
        &self,
        character_id: i64,
    ) -> Result<Option<entity::eve_character::Model>, DbErr> {
        entity::prelude::EveCharacter::find()
            .filter(entity::eve_character::Column::CharacterId.eq(character_id))
            .one(self.db)
            .await
    }
}

#[cfg(test)]
mod tests {

    mod create {
        use bifrost_test_utils::{error::TestError, test_setup, TestSetup};
        use sea_orm::{DbErr, RuntimeErr};

        use crate::server::data::eve::character::CharacterRepository;

        /// Expect success when creating character with a faction ID set
        #[tokio::test]
        async fn returns_success_when_creating_character_with_faction() -> Result<(), TestError> {
            let test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let faction_model = test.insert_mock_faction(1).await?;
            let corporation_model = test.insert_mock_corporation(1, None, None).await?;
            let (character_id, character) =
                test.with_mock_character(1, corporation_model.corporation_id, None, None);

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
        async fn returns_success_when_creating_new_character() -> Result<(), TestError> {
            let test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let corporation_model = test.insert_mock_corporation(1, None, None).await?;
            let (character_id, character) =
                test.with_mock_character(1, corporation_model.corporation_id, None, None);

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
        async fn returns_error_for_character_with_invalid_corporation_id() -> Result<(), TestError>
        {
            let test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let (character_id, character) = test.with_mock_character(1, 1, None, None);

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

    mod get_by_character_id {
        use bifrost_test_utils::{error::TestError, test_setup, TestSetup};

        use crate::server::data::eve::character::CharacterRepository;

        /// Expect Some when character is present in database
        #[tokio::test]
        async fn returns_some_with_existing_character() -> Result<(), TestError> {
            let test = test_setup!(
                entity::prelude::EveFaction,
                entity::prelude::EveAlliance,
                entity::prelude::EveCorporation,
                entity::prelude::EveCharacter
            )?;
            let character_model = test.insert_mock_character(1, 1, None, None).await?;

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
        async fn returns_none_with_non_existant_character() -> Result<(), TestError> {
            let test = test_setup!(
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
        async fn returns_error_with_missing_tables() -> Result<(), TestError> {
            // Use setup function that doesn't create required tables, causing a database error
            let test = test_setup!()?;

            let character_id = 1;
            let character_repo = CharacterRepository::new(&test.state.db);
            let result = character_repo.get_by_character_id(character_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
