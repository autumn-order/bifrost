use chrono::Utc;
use eve_esi::model::universe::Faction;
use sea_orm::{ActiveValue, DatabaseConnection, DbErr, EntityTrait};

pub struct FactionRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> FactionRepository<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        factions: Vec<Faction>,
    ) -> Result<Vec<entity::eve_faction::Model>, DbErr> {
        let factions = factions
            .into_iter()
            .map(|f| entity::eve_faction::ActiveModel {
                faction_id: ActiveValue::Set(f.faction_id),
                corporation_id: ActiveValue::Set(f.corporation_d),
                militia_corporation_id: ActiveValue::Set(f.militia_corporation_id),
                description: ActiveValue::Set(f.description),
                is_unique: ActiveValue::Set(f.is_unique),
                name: ActiveValue::Set(f.name),
                size_factor: ActiveValue::Set(f.size_factor),
                solar_system_id: ActiveValue::Set(f.solar_system_id),
                station_count: ActiveValue::Set(f.faction_id),
                station_system_count: ActiveValue::Set(f.faction_id),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            });

        entity::prelude::EveFaction::insert_many(factions)
            .exec_with_returning(self.db)
            .await
    }
}

#[cfg(test)]
mod tests {
    use eve_esi::model::universe::Faction;
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Schema};

    use crate::server::{
        data::eve::faction::FactionRepository,
        util::test::{eve::mock::mock_faction, setup::test_setup},
    };

    async fn setup() -> Result<DatabaseConnection, DbErr> {
        let test = test_setup().await;

        let db = test.state.db;
        let schema = Schema::new(DbBackend::Sqlite);

        let stmts = vec![schema.create_table_from_entity(entity::prelude::EveFaction)];

        for stmt in stmts {
            db.execute(&stmt).await?;
        }

        Ok(db)
    }

    #[tokio::test]
    async fn create_faction() {
        let db = setup().await.unwrap();
        let repo = FactionRepository::new(&db);

        let faction = mock_faction();
        let result = repo.create(vec![faction]).await;

        assert!(result.is_ok(), "Error: {:?}", result)
    }
}
