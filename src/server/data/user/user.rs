use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, DbErr};

pub struct UserRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserRepository<'a> {
    /// Creates a new instance of [`UserRepository`]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// Creates a new user
    pub async fn create(&self) -> Result<entity::bifrost_user::Model, DbErr> {
        let user = entity::bifrost_user::ActiveModel {
            created_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        user.insert(self.db).await
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

        let stmt = schema.create_table_from_entity(entity::prelude::BifrostUser);

        db.execute(&stmt).await?;

        Ok(db)
    }

    mod create_tests {
        use sea_orm::DbErr;

        use crate::server::{
            data::user::user::{tests::setup, UserRepository},
            util::test::setup::test_setup,
        };

        /// Expect success when creating a new user
        #[tokio::test]
        async fn test_create_user_success() -> Result<(), DbErr> {
            let db = setup().await?;
            let user_repository = UserRepository::new(&db);

            let result = user_repository.create().await;

            assert!(result.is_ok());

            Ok(())
        }

        /// Expect Error when creating a new user without required tables being created
        #[tokio::test]
        async fn test_create_user_error() -> Result<(), DbErr> {
            // Use setup function that does not create required tables, causing database error
            let test = test_setup().await;
            let user_repository = UserRepository::new(&test.state.db);

            let result = user_repository.create().await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
