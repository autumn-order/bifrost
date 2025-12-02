//! User character service layer.
//!
//! This module provides business logic for managing user-character relationships including
//! character ownership linking, transfers between users, and main character management.
//! All operations use transactions to ensure data consistency.

use dioxus_logger::tracing;
use sea_orm::{DatabaseConnection, DatabaseTransaction};

use crate::{
    model::user::{AllianceDto, CharacterDto, CorporationDto},
    server::{
        data::user::{user_character::UserCharacterRepository, UserRepository},
        error::{auth::AuthError, Error},
        model::db::{CharacterOwnershipModel, UserModel},
        service::retry::RetryContext,
    },
};

/// Service for managing user-character ownership operations.
///
/// Provides methods for retrieving user characters with organizational details,
/// linking characters to users, transferring ownership between users, and managing
/// main character assignments. Operations use retry logic and transactions for reliability.
pub struct UserCharacterService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserCharacterService<'a> {
    /// Creates a new instance of UserCharacterService.
    ///
    /// Constructs a service for managing user-character ownership operations.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    ///
    /// # Returns
    /// - `UserCharacterService` - New service instance
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// Retrieves all characters owned by a user with organizational details.
    ///
    /// Fetches complete character information including associated corporation and alliance
    /// data for all characters owned by the specified user. Uses automatic retry logic to
    /// handle transient database failures.
    ///
    /// # Arguments
    /// - `user_id` - ID of the user whose characters to retrieve
    ///
    /// # Returns
    /// - `Ok(Vec<CharacterDto>)` - List of characters with corporation and alliance information
    /// - `Err(Error::DbErr)` - Database operation failed after retries
    pub async fn get_user_characters(&self, user_id: i32) -> Result<Vec<CharacterDto>, Error> {
        let mut ctx: RetryContext<()> = RetryContext::new();

        let db = self.db.clone();

        ctx.execute_with_retry(&format!("get characters for user ID {}", user_id), |_| {
            let db = db.clone();

            Box::pin(async move {
                let user_characters = UserCharacterRepository::new(&db)
                    .get_owned_characters_by_user_id(user_id)
                    .await?;

                let character_dtos: Vec<CharacterDto> = user_characters
                    .into_iter()
                    .map(|(character, corporation, alliance)| {
                        let alliance_dto = if let Some(alliance) = alliance {
                            Some(AllianceDto {
                                id: alliance.alliance_id,
                                name: alliance.name.clone(),
                                updated_at: alliance.updated_at,
                            })
                        } else {
                            None
                        };

                        CharacterDto {
                            id: character.character_id,
                            name: character.name,
                            corporation: CorporationDto {
                                id: corporation.corporation_id,
                                name: corporation.name.clone(),
                                info_updated_at: corporation.info_updated_at,
                                affiliation_updated_at: corporation.affiliation_updated_at,
                            },
                            alliance: alliance_dto,
                            info_updated_at: character.info_updated_at,
                            affiliation_updated_at: character.affiliation_updated_at,
                        }
                    })
                    .collect();

                Ok(character_dtos)
            })
        })
        .await
    }

    /// Links a character to a user or updates existing ownership.
    ///
    /// Creates or updates the character ownership record, associating the character
    /// with the specified user and updating the owner hash for verification. This operation
    /// must be executed within a transaction provided by the caller.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to execute the operation within
    /// - `character_record_id` - Internal database ID of the character record
    /// - `to_user_id` - ID of the user to link the character to
    /// - `owner_hash` - EVE Online owner hash from JWT token for ownership verification
    ///
    /// # Returns
    /// - `Ok(CharacterOwnershipModel)` - The created or updated ownership record
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn link_character(
        txn: &DatabaseTransaction,
        character_record_id: i32,
        to_user_id: i32,
        owner_hash: &str,
    ) -> Result<CharacterOwnershipModel, Error> {
        let user_character_repo = UserCharacterRepository::new(txn);

        let ownership = user_character_repo
            .upsert(character_record_id, to_user_id, owner_hash.to_string())
            .await?;

        Ok(ownership)
    }

    /// Transfers character ownership from one user to another.
    ///
    /// Handles the complete transfer process including verification, ownership updates, and
    /// cleanup. The process verifies the previous user exists, updates their main character
    /// if necessary (or deletes the user if no characters remain), and links the character
    /// to the new user. This operation must be executed within a transaction.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to execute the operation within
    /// - `character_record_id` - Internal database ID of the character record
    /// - `to_user_id` - ID of the user to transfer the character to
    /// - `owner_hash` - EVE Online owner hash from JWT token for ownership verification
    ///
    /// # Returns
    /// - `Ok(CharacterOwnershipModel)` - The updated ownership record
    /// - `Err(Error::AuthError(AuthError::UserNotInDatabase))` - Previous user not found in database
    /// - `Err(Error::AuthError(AuthError::CharacterNotOwned))` - Character has no current ownership
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn transfer_character(
        txn: &DatabaseTransaction,
        character_record_id: i32,
        to_user_id: i32,
        owner_hash: &str,
    ) -> Result<CharacterOwnershipModel, Error> {
        let user_repo = UserRepository::new(txn);
        let user_character_repo = UserCharacterRepository::new(txn);

        // Get current ownership to find the actual owner
        let ownership = user_character_repo
            .get_ownership_by_character_id(character_record_id)
            .await?
            .ok_or_else(|| Error::AuthError(AuthError::CharacterNotOwned))?;

        let from_user_id = ownership.user_id;

        // Retrieve user information to check if main character change is needed
        let Some((prev_user, maybe_main_character)) = user_repo.get_by_id(from_user_id).await?
        else {
            return Err(Error::AuthError(AuthError::UserNotInDatabase(from_user_id)));
        };

        // Use link_character method to update ownership to provided user ID
        let ownership =
            Self::link_character(&txn, character_record_id, to_user_id, &owner_hash).await?;

        // Handle main character change if:
        // 1. Character is being transferred to a different user
        // 2. The character being transferred was the previous user's main
        if prev_user.id != to_user_id && prev_user.main_character_id == character_record_id {
            let prev_user_character_ids: Vec<i32> = user_character_repo
                .get_ownerships_by_user_id(prev_user.id)
                .await?
                .into_iter()
                .map(|c| c.id)
                .collect();

            // Find another character to set as main, or delete the user if none exist
            let alternative_character = prev_user_character_ids
                .into_iter()
                .find(|id| *id != character_record_id);

            match alternative_character {
                Some(new_main_character_id) => {
                    user_repo
                        .update(prev_user.id, new_main_character_id)
                        .await?;
                }
                None => {
                    if let Some(character) = maybe_main_character {
                        tracing::info!(
                            deleted_user_id = %prev_user.id,
                            character_id = %character.character_id,
                            character_name = %character.name,
                            new_owner_id = %to_user_id,
                            "Deleted user after transferring their only remaining character to another user"
                        )
                    } else {
                        // Only occurs if foreign-key constraint requiring user's main character to
                        // exist in database is not properly enforced.
                        tracing::warn!(
                            deleted_user_id = %prev_user.id,
                            new_owner_id = %to_user_id,
                            character_record_id = %character_record_id,
                            "Deleted user after transferring their only remaining character to another user. Could not retrieve character information from database, likely due to FK constraint violation."
                        )
                    }

                    user_repo.delete(prev_user.id).await?;
                }
            }
        }

        Ok(ownership)
    }

    /// Sets a character as the user's main character.
    ///
    /// Updates the user's main character after verifying that the character is actually
    /// owned by the specified user. This prevents unauthorized changes to main character
    /// assignments. This operation must be executed within a transaction.
    ///
    /// # Arguments
    /// - `txn` - Database transaction to execute the operation within
    /// - `user_id` - ID of the user whose main character should be updated
    /// - `ownership` - Ownership record of the character to set as main
    ///
    /// # Returns
    /// - `Ok(Some(UserModel))` - The updated user record
    /// - `Ok(None)` - User was not found (should be unreachable in normal operation)
    /// - `Err(Error::AuthError(AuthError::CharacterOwnedByAnotherUser))` - Character is owned by a different user
    /// - `Err(Error::DbErr)` - Database operation failed
    pub async fn set_main_character(
        txn: &DatabaseTransaction,
        user_id: i32,
        ownership: CharacterOwnershipModel,
    ) -> Result<Option<UserModel>, Error> {
        let user_repo = UserRepository::new(txn);

        if ownership.user_id != user_id {
            tracing::warn!(
                user_id = %user_id,
                character_id = %ownership.character_id,
                actual_owner_id = %ownership.user_id,
                "User attempted to change main to character owned by another user"
            );

            return Err(AuthError::CharacterOwnedByAnotherUser.into());
        }

        let user = user_repo.update(user_id, ownership.character_id).await?;

        Ok(user)
    }
}
