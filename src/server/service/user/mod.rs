pub mod user_character;

#[cfg(test)]
mod tests;

use dioxus_logger::tracing;
use eve_esi::model::oauth2::EveJwtClaims;
use sea_orm::DatabaseConnection;

use crate::{
    model::user::UserDto,
    server::{
        data::user::{user_character::UserCharacterRepository, UserRepository},
        error::Error,
        service::{eve::character::CharacterService, user::user_character::UserCharacterService},
    },
};

pub struct UserService {
    db: DatabaseConnection,
    esi_client: eve_esi::Client,
}

impl UserService {
    /// Creates a new instance of [`UserService`]
    pub fn new(db: DatabaseConnection, esi_client: eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    pub async fn get_or_create_user(&self, claims: EveJwtClaims) -> Result<i32, Error> {
        let user_repo = UserRepository::new(self.db.clone());
        let user_character_repo = UserCharacterRepository::new(&self.db);
        let character_service = CharacterService::new(self.db.clone(), self.esi_client.clone());
        let user_character_service =
            UserCharacterService::new(self.db.clone(), self.esi_client.clone());

        let character_id = claims.character_id()?;
        let character = match user_character_repo
            .get_by_character_id(character_id)
            .await?
        {
            Some((character, maybe_owner)) => {
                if let Some(ownership_entry) = maybe_owner {
                    // Validate whether or not character has been sold or transferred between accounts
                    if claims.owner == ownership_entry.owner_hash {
                        tracing::trace!(
                            "User ID {} returned for character ID {} - ownership verified",
                            ownership_entry.user_id,
                            character_id
                        );

                        // User ownership hasn't changed, user still owns this character
                        return Ok(ownership_entry.user_id);
                    }

                    // Character has been sold or transferred, create a new user account
                    let new_user = user_repo.create(ownership_entry.character_id).await?;
                    user_character_service
                        .transfer_character(ownership_entry, new_user.id)
                        .await?;

                    tracing::trace!(
                        "New user ID {} created for character ID {} - ownership transfer detected",
                        new_user.id,
                        character_id
                    );

                    return Ok(new_user.id);
                }

                // Character exists but not owned by any user, no need to create the character
                character
            }
            // Character not found in database, create the character
            None => character_service.create_character(character_id).await?,
        };

        // Create new user and link character to user
        let new_user = user_repo.create(character.id).await?;
        let _ = user_character_repo
            .create(new_user.id, character.id, claims.owner)
            .await?;

        tracing::trace!(
            "New user ID {} created for character ID {} - first login",
            new_user.id,
            character.id
        );

        Ok(new_user.id)
    }

    /// Retrieves main character for provided user ID
    pub async fn get_user(&self, user_id: i32) -> Result<Option<UserDto>, Error> {
        let user_repo = UserRepository::new(self.db.clone());

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

    /// Deletes the provided user ID
    ///
    /// # Warning
    /// This will error if you attempt to delete the user while they still have
    /// connected character ownerships, you must [`Self::transfer_character`] first
    /// to another user before deleting a user.
    pub async fn delete_user(&self, user_id: i32) -> Result<bool, Error> {
        let user_repo = UserRepository::new(self.db.clone());

        let delete_result = user_repo.delete(user_id).await?;

        Ok(delete_result.rows_affected == 1)
    }
}
