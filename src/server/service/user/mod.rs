pub mod user_character;

#[cfg(test)]
mod tests;

use sea_orm::DatabaseConnection;

use crate::{
    model::user::UserDto,
    server::{data::user::UserRepository, error::Error, service::retry::RetryContext},
};

/// Service for managing user operations.
///
/// Provides methods for retrieving user information and their associated
/// main character details.
pub struct UserService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserService<'a> {
    /// Creates a new instance of [`UserService`]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// Gets user information including their main character details.
    ///
    /// This method retrieves the user record and their associated main character information.
    /// The operation uses automatic retry logic to handle transient database failures.
    ///
    /// # Arguments
    /// - `user_id` - ID of the user to retrieve
    ///
    /// # Returns
    /// - `Ok(Some(UserDto))` - User found with main character information
    /// - `Ok(None)` - User not found in database
    /// - `Err(Error::DbErr)` - Database operation failed after retries, or main character record not found (FK constraint violation)
    pub async fn get_user(&self, user_id: i32) -> Result<Option<UserDto>, Error> {
        let mut ctx: RetryContext<()> = RetryContext::new();

        let db = self.db.clone();

        ctx.execute_with_retry(&format!("get user ID {}", user_id), |_| {
            let db = db.clone();

            Box::pin(async move {
                let user_repo = UserRepository::new(&db);

                match user_repo.get_by_id(user_id).await? {
                    None => return Ok(None),
                    Some((user, maybe_main_character)) => {
                        let main_character = maybe_main_character.ok_or_else(|| {
                            // Would only occur if the foreign key constraint requiring main character to exist in
                            // database for the user is not properly enforced
                            Error::InternalError(format!(
                                "Failed to find main character information for user ID {} with main character ID {}",
                                user.id, user.main_character_id
                            ))
                        })?;

                        Ok(Some(UserDto {
                            id: user.id,
                            character_id: main_character.character_id,
                            character_name: main_character.name,
                        }))
                    }
                }
            })
        })
        .await
    }
}
