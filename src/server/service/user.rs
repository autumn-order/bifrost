use sea_orm::DatabaseConnection;

pub struct UserService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserService<'a> {
    /// Creates a new instance of [`UserService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db }
    }
}
