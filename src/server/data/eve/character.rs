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
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Schema};

    use crate::server::{
        data::eve::{
            alliance::AllianceRepository, character::CharacterRepository,
            corporation::CorporationRepository, faction::FactionRepository,
        },
        util::test::{
            eve::mock::{mock_alliance, mock_character, mock_corporation, mock_faction},
            setup::test_setup,
        },
    };

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

    #[tokio::test]
    async fn create_character() {
        let db = setup().await.unwrap();
        let faction_repo = FactionRepository::new(&db);
        let alliance_repo = AllianceRepository::new(&db);
        let corporation_repo = CorporationRepository::new(&db);
        let character_repo = CharacterRepository::new(&db);

        let faction = mock_faction();
        let alliance_id = 1;
        let alliance = mock_alliance(Some(faction.faction_id));
        let corporation_id = 1;
        let corporation = mock_corporation(Some(alliance_id), Some(faction.faction_id));

        let faction_result = faction_repo.create(vec![faction]).await;

        assert!(faction_result.is_ok(), "Error: {:?}", faction_result);
        let faction = faction_result.unwrap().first().unwrap().to_owned();

        let alliance_result = alliance_repo
            .create(alliance_id, alliance, Some(faction.id))
            .await;

        assert!(alliance_result.is_ok(), "Error: {:?}", alliance_result);
        let alliance = alliance_result.unwrap();

        let corporation_result = corporation_repo
            .create(
                corporation_id,
                corporation,
                Some(alliance.id),
                Some(faction.id),
            )
            .await;

        assert!(
            corporation_result.is_ok(),
            "Error: {:?}",
            corporation_result
        );
        let corporation = corporation_result.unwrap();

        let character_id = 1;
        let character = mock_character();
        let result = character_repo
            .create(character_id, character, corporation.id, Some(faction.id))
            .await;

        assert!(result.is_ok(), "Error: {:?}", result)
    }
}
