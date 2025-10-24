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
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, RuntimeErr, Schema};

    use crate::server::{
        data::eve::{
            alliance::AllianceRepository, character::CharacterRepository,
            corporation::CorporationRepository, faction::FactionRepository,
        },
        util::test::{
            eve::mock::{mock_alliance, mock_corporation, mock_faction},
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

    /// Inserts a mock faction, alliance, and corporation for foreign key dependencies
    async fn insert_foreign_key_dependencies(
        db: &DatabaseConnection,
    ) -> (entity::eve_corporation::Model, entity::eve_faction::Model) {
        let faction_repo = FactionRepository::new(&db);
        let alliance_repo = AllianceRepository::new(&db);
        let corporation_repo = CorporationRepository::new(&db);

        let faction = mock_faction();
        let alliance_id = 1;
        let alliance = mock_alliance(Some(faction.faction_id));
        let corporation_id = 1;
        let corporation = mock_corporation(Some(alliance_id), Some(faction.faction_id));

        let faction = faction_repo
            .upsert_many(vec![faction])
            .await
            .unwrap()
            .first()
            .unwrap()
            .to_owned();

        let alliance = alliance_repo
            .create(alliance_id, alliance, Some(faction.id))
            .await
            .unwrap();

        let corporation = corporation_repo
            .create(
                corporation_id,
                corporation,
                Some(alliance.id),
                Some(faction.id),
            )
            .await
            .unwrap();

        (corporation, faction)
    }

    fn mock_character() -> eve_esi::model::character::Character {
        use crate::server::util::test::eve::mock::mock_character;

        let corporation_id = 1;
        let alliance_id = None;
        let faction_id = None;

        mock_character(corporation_id, alliance_id, faction_id)
    }

    /// Should succeed when inserting character into table with faction ID
    #[tokio::test]
    async fn create_character() {
        let db = setup().await.unwrap();
        let (corporation, faction) = insert_foreign_key_dependencies(&db).await;

        let character_repo = CharacterRepository::new(&db);

        let character_id = 1;
        let character = mock_character();
        let result = character_repo
            .create(character_id, character, corporation.id, Some(faction.id))
            .await;

        assert!(result.is_ok(), "Error: {:?}", result);
        let created = result.unwrap();

        // Need to create mock character again as eve_esi::model::character::Character does not implement Clone
        // - An issue will need to be made on the eve_esi repo about this
        let character = mock_character();

        assert_eq!(created.character_id, character_id, "character_id mismatch");
        assert_eq!(created.name, character.name, "name mismatch");
        assert_eq!(
            created.corporation_id, corporation.id,
            "corporation_id mismatch"
        );
        assert_eq!(created.faction_id, Some(faction.id), "faction_id mismatch");
    }

    /// Should succeed when inserting character into table without faction ID
    #[tokio::test]
    async fn create_character_no_faction() {
        let db = setup().await.unwrap();

        let alliance_repo = AllianceRepository::new(&db);
        let corporation_repo = CorporationRepository::new(&db);

        let alliance_id = 1;
        let alliance = mock_alliance(None);
        let corporation_id = 1;
        let corporation = mock_corporation(Some(alliance_id), None);

        let alliance = alliance_repo
            .create(alliance_id, alliance, None)
            .await
            .unwrap();

        let corporation = corporation_repo
            .create(corporation_id, corporation, Some(alliance.id), None)
            .await
            .unwrap();

        let character_repo = CharacterRepository::new(&db);

        let character_id = 1;
        let character = mock_character();
        let result = character_repo
            .create(character_id, character, corporation.id, None)
            .await;

        assert!(result.is_ok(), "Error: {:?}", result);
        let created = result.unwrap();

        assert_eq!(created.faction_id, None, "faction_id mismatch");
    }

    /// Should error when inserting character into table without a valid corporation
    #[tokio::test]
    async fn create_character_no_corporation_error() {
        let db = setup().await.unwrap();

        let character_repo = CharacterRepository::new(&db);

        // Create a character using corporation ID that does not exist in database
        let non_existant_corporation_id = 1;
        let character_id = 1;
        let character = mock_character();
        let result = character_repo
            .create(character_id, character, non_existant_corporation_id, None)
            .await;

        assert!(result.is_err(), "Expected error, instead got: {:?}", result);

        // Assert error code is 787 indicating a foreign key constraint failure
        let code = result.err().and_then(|e| match e {
            DbErr::Query(RuntimeErr::SqlxError(se)) => se
                .as_database_error()
                .and_then(|d| d.code().map(|c| c.to_string())),
            _ => None,
        });
        assert_eq!(code.as_deref(), Some("787"));
    }
}
