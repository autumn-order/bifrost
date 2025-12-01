pub mod user_character;

#[cfg(test)]
mod tests;

use sea_orm::DatabaseConnection;

use crate::{
    model::user::UserDto,
    server::{data::user::UserRepository, error::Error},
};

pub struct UserService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserService<'a> {
    /// Creates a new instance of [`UserService`]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// Retrieves main character for provided user ID
    pub async fn get_user(&self, user_id: i32) -> Result<Option<UserDto>, Error> {
        let user_repo = UserRepository::new(self.db);

        match user_repo.get(user_id).await? {
            None => return Ok(None),
            Some((user, maybe_main_character)) => {
                let main_character = maybe_main_character.ok_or_else(|| {
                    // Should not occur due to foreign key constraint requiring main character to exist
                    Error::DbErr(sea_orm::DbErr::RecordNotFound(format!(
                        "Failed to find main character information for user ID {} with main character ID {}",
                        user.id, user.main_character_id
                    )))
                })?;

                Ok(Some(UserDto {
                    id: user.id,
                    character_id: main_character.character_id,
                    character_name: main_character.name,
                }))
            }
        }
    }
}
