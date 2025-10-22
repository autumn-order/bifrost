use chrono::Utc;
use eve_esi::model::corporation::Corporation;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, DbErr};

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
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Schema};

    use crate::server::{
        data::eve::{
            alliance::AllianceRepository, corporation::CorporationRepository,
            faction::FactionRepository,
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
        ];

        for stmt in stmts {
            db.execute(&stmt).await?;
        }

        Ok(db)
    }

    /// Inserts a mock faction & alliance for foreign key dependencies
    async fn insert_foreign_key_dependencies(
        db: &DatabaseConnection,
    ) -> (entity::eve_alliance::Model, entity::eve_faction::Model) {
        let faction_repo = FactionRepository::new(&db);
        let alliance_repo = AllianceRepository::new(&db);

        let faction = mock_faction();
        let alliance_id = 1;
        let alliance = mock_alliance(Some(faction.faction_id));

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

        (alliance, faction)
    }

    // Should succeed when inserting a corporation with both an alliance & faction ID
    #[tokio::test]
    async fn create_corporation() {
        let db = setup().await.unwrap();
        let (alliance, faction) = insert_foreign_key_dependencies(&db).await;

        let corporation_repo = CorporationRepository::new(&db);

        let corporation_id = 1;
        let corporation = mock_corporation(Some(alliance.alliance_id), Some(faction.faction_id));
        let result = corporation_repo
            .create(
                corporation_id,
                corporation,
                Some(alliance.id),
                Some(faction.id),
            )
            .await;

        assert!(result.is_ok(), "Error: {:?}", result);
        let created = result.unwrap();

        // Need to create mock corporation again as eve_esi::model::corporation::Corporation does not implement Clone
        // - An issue will need to be made on the eve_esi repo about this
        let corporation = mock_corporation(Some(alliance.alliance_id), Some(faction.faction_id));

        assert_eq!(
            created.corporation_id, corporation_id,
            "corporation_id mismatch"
        );
        assert_eq!(created.name, corporation.name, "name mismatch");
        assert_eq!(
            created.alliance_id,
            Some(alliance.id),
            "alliance_id mismatch"
        );
        assert_eq!(created.faction_id, Some(faction.id), "faction_id mismatch");
    }

    /// Should succeed when inserting corporation into table without an alliance ID
    #[tokio::test]
    async fn create_corporation_no_alliance() {
        let db = setup().await.unwrap();

        let faction_repo = FactionRepository::new(&db);

        let faction = faction_repo
            .upsert_many(vec![mock_faction()])
            .await
            .unwrap()
            .first()
            .unwrap()
            .to_owned();

        let corporation_repo = CorporationRepository::new(&db);

        let corporation_id = 1;
        let corporation = mock_corporation(None, Some(faction.faction_id));
        let result = corporation_repo
            .create(corporation_id, corporation, None, Some(faction.id))
            .await;

        assert!(result.is_ok(), "Error: {:?}", result);
        let created = result.unwrap();

        assert_eq!(created.alliance_id, None);
        assert_eq!(created.faction_id, Some(faction.id))
    }

    /// Should succeed when inserting corporation into table without a faction or alliance ID
    #[tokio::test]
    async fn create_corporation_no_alliance_no_faction() {
        let db = setup().await.unwrap();

        let corporation_repo = CorporationRepository::new(&db);

        let corporation_id = 1;
        let corporation = mock_corporation(None, None);
        let result = corporation_repo
            .create(corporation_id, corporation, None, None)
            .await;

        assert!(result.is_ok(), "Error: {:?}", result);
        let created = result.unwrap();

        assert_eq!(created.alliance_id, None);
        assert_eq!(created.faction_id, None)
    }
}
