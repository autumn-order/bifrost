use sea_orm::DatabaseConnection;

pub struct UserRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserRepository<'a> {
    /// Creates a new instance of [`UserRepository`]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }
}
