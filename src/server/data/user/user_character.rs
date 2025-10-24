use sea_orm::DatabaseConnection;

pub struct UserCharacterRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserCharacterRepository<'a> {
    /// Creates a new instance of [`UserCharacterRepository`]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }
}
