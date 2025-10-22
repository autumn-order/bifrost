pub use sea_orm_migration::prelude::*;

mod m20251017_000001_create_eve_faction_table;
mod m20251017_000002_create_eve_alliance_table;
mod m20251017_000003_create_eve_corporation_table;
mod m20251017_000004_create_eve_character_table;
mod m20251017_000005_create_bifrost_auth_user_table;
mod m20251017_000006_create_bifrost_auth_user_character_table;
mod m20251017_000007_create_bifrost_auth_user_character_history_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251017_000001_create_eve_faction_table::Migration),
            Box::new(m20251017_000002_create_eve_alliance_table::Migration),
            Box::new(m20251017_000003_create_eve_corporation_table::Migration),
            Box::new(m20251017_000004_create_eve_character_table::Migration),
            Box::new(m20251017_000005_create_bifrost_auth_user_table::Migration),
            Box::new(m20251017_000006_create_bifrost_auth_user_character_table::Migration),
            Box::new(m20251017_000007_create_bifrost_auth_user_character_history_table::Migration),
        ]
    }
}
