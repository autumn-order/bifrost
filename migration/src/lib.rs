pub use sea_orm_migration::prelude::*;

mod m20251017_000001_eve_faction;
mod m20251017_000002_eve_alliance;
mod m20251017_000003_eve_corporation;
mod m20251017_000004_eve_character;
mod m20251017_000005_bifrost_auth_user;
mod m20251017_000006_bifrost_auth_user_character;
mod m20251017_000007_bifrost_auth_user_character_history;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251017_000001_eve_faction::Migration),
            Box::new(m20251017_000002_eve_alliance::Migration),
            Box::new(m20251017_000003_eve_corporation::Migration),
            Box::new(m20251017_000004_eve_character::Migration),
            Box::new(m20251017_000005_bifrost_auth_user::Migration),
            Box::new(m20251017_000006_bifrost_auth_user_character::Migration),
            Box::new(m20251017_000007_bifrost_auth_user_character_history::Migration),
        ]
    }
}
