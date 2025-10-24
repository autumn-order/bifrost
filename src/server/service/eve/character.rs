use sea_orm::DatabaseConnection;

pub struct CharacterService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CharacterService<'a> {
    /// Creates a new instance of [`CharacterService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }
}
