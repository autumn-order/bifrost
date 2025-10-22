use chrono::Utc;
use eve_esi::model::alliance::Alliance;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, DbErr};

pub struct AllianceRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> AllianceRepository<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        alliance_id: i64,
        alliance: Alliance,
        faction_id: Option<i32>,
    ) -> Result<entity::eve_alliance::Model, DbErr> {
        let alliance = entity::eve_alliance::ActiveModel {
            alliance_id: ActiveValue::Set(alliance_id),
            faction_id: ActiveValue::Set(faction_id),
            creator_corporation_id: ActiveValue::Set(alliance.creator_corporation_id),
            executor_corporation_id: ActiveValue::Set(alliance.executor_corporation_id),
            creator_id: ActiveValue::Set(alliance.creator_id),
            date_founded: ActiveValue::Set(alliance.date_founded.naive_utc()),
            name: ActiveValue::Set(alliance.name),
            ticker: ActiveValue::Set(alliance.ticker),
            created_at: ActiveValue::Set(Utc::now().naive_utc()),
            updated_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        alliance.insert(self.db).await
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Schema};

    use crate::server::{
        data::eve::{alliance::AllianceRepository, faction::FactionRepository},
        util::test::{
            eve::mock::{mock_alliance, mock_faction},
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
        ];

        for stmt in stmts {
            db.execute(&stmt).await?;
        }

        Ok(db)
    }

    /// Inserts a mock faction for foreign key dependencies
    async fn insert_foreign_key_dependencies(
        db: &DatabaseConnection,
    ) -> entity::eve_faction::Model {
        let faction_repo = FactionRepository::new(&db);

        let faction = mock_faction();

        let faction = faction_repo
            .create(vec![faction])
            .await
            .unwrap()
            .first()
            .unwrap()
            .to_owned();

        faction
    }

    #[tokio::test]
    async fn create_alliance() {
        let db = setup().await.unwrap();
        let faction = insert_foreign_key_dependencies(&db).await;

        let alliance_repo = AllianceRepository::new(&db);

        let alliance_id = 1;
        let alliance = mock_alliance(Some(faction.faction_id));
        let result = alliance_repo
            .create(alliance_id, alliance, Some(faction.id))
            .await;

        assert!(result.is_ok(), "Error: {:?}", result);
        let created = result.unwrap();

        // Need to create mock alliance again as eve_esi::model::alliance::Alliance does not implement Clone
        // - An issue will need to be made on the eve_esi repo about this
        let alliance = mock_alliance(Some(faction.faction_id));

        assert_eq!(created.alliance_id, alliance_id, "alliance_id mismatch");
        assert_eq!(created.name, alliance.name, "name mismatch");
        assert_eq!(created.faction_id, Some(faction.id), "faction_id mismatch");
    }
}
