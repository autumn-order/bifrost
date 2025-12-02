//! User service layer.
//!
//! This module contains business logic services for user operations including
//! user account management and character ownership. Services coordinate between
//! repositories and handle complex multi-step operations with retry logic.

pub mod user_character;

#[cfg(test)]
mod tests;

use sea_orm::DatabaseConnection;

use crate::{
    model::user::UserDto,
    server::{data::user::UserRepository, error::Error, service::retry::RetryContext},
};

/// Service for managing user account operations.
///
/// Provides methods for retrieving user information and their associated
/// main character details. Operations use automatic retry logic for transient failures.
pub struct UserService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserService<'a> {
    /// Creates a new instance of UserService.
    ///
    /// Constructs a service for managing user account operations.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    ///
    /// # Returns
    /// - `UserService` - New service instance
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// Retrieves user information with their main character details.
    ///
    /// Fetches the user record and associated main character information from the database.
    /// Uses automatic retry logic to handle transient database failures.
    ///
    /// # Arguments
    /// - `user_id` - ID of the user to retrieve
    ///
    /// # Returns
    /// - `Ok(Some(UserDto))` - User found with main character information
    /// - `Ok(None)` - User not found in database
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    /// - `Err(Error::InternalError)` - Main character record not found (FK constraint violation)
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
