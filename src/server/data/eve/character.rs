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
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Schema};

    use crate::server::util::test::setup::test_setup;

    async fn setup() -> Result<DatabaseConnection, DbErr> {
        let test = test_setup().await;

        let db = test.state.db;
        let schema = Schema::new(DbBackend::Sqlite);

        let stmts = vec![
            schema.create_table_from_entity(entity::prelude::EveFaction),
            schema.create_table_from_entity(entity::prelude::EveAlliance),
            schema.create_table_from_entity(entity::prelude::EveCorporation),
            schema.create_table_from_entity(entity::prelude::EveCharacter),
        ];

        for stmt in stmts {
            db.execute(&stmt).await?;
        }

        Ok(db)
    }

    fn mock_character() -> eve_esi::model::character::Character {
        use crate::server::util::test::eve::mock::mock_character;

        let corporation_id = 1;
        let alliance_id = None;
        let faction_id = None;

        mock_character(corporation_id, alliance_id, faction_id)
    }

    mod create_character_tests {
        use sea_orm::{DbErr, RuntimeErr};

        use crate::server::{
            data::eve::{
                character::{
                    tests::{mock_character, setup},
                    CharacterRepository,
                },
                corporation::CorporationRepository,
                faction::FactionRepository,
            },
            util::test::eve::mock::{mock_corporation, mock_faction},
        };

        /// Expect success when creating character entry
        #[tokio::test]
        async fn test_create_character() {
            let db = setup().await.unwrap();
            let character_repo = CharacterRepository::new(&db);
            let corporation_repo = CorporationRepository::new(&db);

            let corporation_id = 1;
            let alliance_id = None;
            let corporation = mock_corporation(alliance_id, None);

            let corporation = corporation_repo
                .create(corporation_id, corporation, None, None)
                .await
                .unwrap();

            let character_id = 1;
            let character = mock_character();
            let result = character_repo
                .create(character_id, character, corporation.id, None)
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();

            assert_eq!(created.faction_id, None);
        }

        /// Expect success when creating character with a faction ID set
        #[tokio::test]
        async fn test_create_character_with_faction() -> Result<(), DbErr> {
            let db = setup().await?;
            let character_repo = CharacterRepository::new(&db);
            let corporation_repo = CorporationRepository::new(&db);
            let faction_repo = FactionRepository::new(&db);

            let mock_faction = mock_faction();

            let corporation_id = 1;
            let alliance_id = None;
            let mock_corporation = mock_corporation(alliance_id, None);

            let character_id = 1;
            let mock_character = mock_character();

            let faction = faction_repo
                .upsert_many(vec![mock_faction])
                .await?
                .first()
                .unwrap()
                .to_owned();

            let corporation = corporation_repo
                .create(corporation_id, mock_corporation, None, None)
                .await?;

            let result = character_repo
                .create(
                    character_id,
                    mock_character,
                    corporation.id,
                    Some(faction.id),
                )
                .await;

            assert!(result.is_ok());
            let created = result.unwrap();

            assert_eq!(created.character_id, character_id);
            assert_eq!(created.corporation_id, corporation.id);
            assert_eq!(created.faction_id, Some(faction.id));

            Ok(())
        }

        /// Expect error when attempting to create a character without a valid corporation ID set
        #[tokio::test]
        async fn test_create_character_without_valid_corporation() {
            let db = setup().await.unwrap();
            let character_repo = CharacterRepository::new(&db);

            let character_id = 1;
            let character = mock_character();

            // Create a character using corporation ID that does not exist in database
            let non_existant_corporation_id = 1;
            let result = character_repo
                .create(character_id, character, non_existant_corporation_id, None)
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
        }
    }

    mod get_by_character_id_tests {
        use sea_orm::DbErr;

        use crate::server::{
            data::eve::{
                character::{
                    tests::{mock_character, setup},
                    CharacterRepository,
                },
                corporation::CorporationRepository,
            },
            util::test::{eve::mock::mock_corporation, setup::test_setup},
        };

        // Expect Some when character entry is present
        #[tokio::test]
        async fn test_get_by_character_id_some() -> Result<(), DbErr> {
            let db = setup().await.unwrap();
            let character_repo = CharacterRepository::new(&db);
            let corporation_repo = CorporationRepository::new(&db);

            let corporation_id = 1;
            let alliance_id = None;
            let corporation = mock_corporation(alliance_id, None);

            let character_id = 1;
            let character = mock_character();

            // Create required corporation & character entry
            let corporation = corporation_repo
                .create(corporation_id, corporation, None, None)
                .await
                .unwrap();

            let character = character_repo
                .create(character_id, character, corporation.id, None)
                .await?;

            let result = character_repo
                .get_by_character_id(character.character_id)
                .await;

            assert!(result.is_ok());
            let character_option = result.unwrap();

            assert!(character_option.is_some());

            Ok(())
        }

        // Expect None when no character entry is present
        #[tokio::test]
        async fn test_get_by_character_id_none() -> Result<(), DbErr> {
            let db = setup().await.unwrap();
            let character_repo = CharacterRepository::new(&db);

            let character_id = 1;
            let result = character_repo.get_by_character_id(character_id).await;

            assert!(result.is_ok());
            let character_option = result.unwrap();

            assert!(character_option.is_none());

            Ok(())
        }

        // Expect Error when required database tables have not been created
        #[tokio::test]
        async fn test_get_by_character_id_error() -> Result<(), DbErr> {
            // Use setup function that doesn't create required tables, causing a database error
            let test = test_setup().await;
            let character_repo = CharacterRepository::new(&test.state.db);

            let character_id = 1;
            let result = character_repo.get_by_character_id(character_id).await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
