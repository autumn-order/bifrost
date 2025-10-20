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
    ) -> Result<entity::eve_character::Model, DbErr> {
        let character = entity::eve_character::ActiveModel {
            character_id: ActiveValue::Set(character_id),
            corporation_id: ActiveValue::Set(character.corporation_id),
            faction_id: ActiveValue::Set(character.faction_id),
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

    pub async fn get_by_id(&self, id: i32) -> Result<Option<entity::eve_character::Model>, DbErr> {
        entity::prelude::EveCharacter::find_by_id(id)
            .one(self.db)
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
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use eve_esi::model::character::Character;
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Schema};

    use crate::server::{data::eve::character::CharacterRepository, util::test::setup::test_setup};

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

    fn mock_character() -> Character {
        Character {
            alliance_id: Some(99013534),
            birthday: DateTime::parse_from_rfc3339("2018-12-20T16:11:54Z")
                .unwrap()
                .with_timezone(&Utc),
            bloodline_id: 7,
            corporation_id: 98785281,
            description: Some("description".to_string()),
            faction_id: None,
            gender: "male".to_string(),
            name: "Hyziri".to_string(),
            race_id: 8,
            security_status: Some(-0.100373643),
            title: Some("Title".to_string()),
        }
    }

    #[tokio::test]
    async fn create_character() {
        let db = setup().await.unwrap();
        let repo = CharacterRepository::new(&db);

        let character_id = 1;
        let character = mock_character();

        let result = repo.create(character_id, character).await;

        assert!(result.is_ok(), "Error: {:?}", result)
    }
}
