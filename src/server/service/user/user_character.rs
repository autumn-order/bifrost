use dioxus_logger::tracing;
use eve_esi::model::oauth2::EveJwtClaims;
use sea_orm::DatabaseConnection;

use crate::{
    model::user::{AllianceDto, CharacterDto, CorporationDto},
    server::{
        data::user::{user_character::UserCharacterRepository, UserRepository},
        error::{auth::AuthError, Error},
        service::{eve::character::CharacterService, user::UserService},
    },
};

pub struct UserCharacterService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> UserCharacterService<'a> {
    /// Creates a new instance of [`UserService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Gets character information for all characters owned by the user,
    /// including their corporation and alliance details.
    ///
    /// Characters without a corporation (which violates foreign key constraint)
    /// will be logged as warnings and skipped from the results.
    pub async fn get_user_characters(&self, user_id: i32) -> Result<Vec<CharacterDto>, Error> {
        let user_characters = UserCharacterRepository::new(&self.db)
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
    }

    /// Links or transfers character to provided user ID
    ///
    /// # Behavior
    /// - If the character is already linked to the provided user (owner hash matches `claims.owner` &
    ///   user ID matches the logged in user ID), no action is taken and the method returns `Ok(false)`.
    /// - If the character is linked to a different owner hash or user ID, the method returns `Ok(true)`
    ///   to indicate a transfer to the provided user ID
    /// - If the character exists but has no owner, a link is created that associates the
    ///   character with the provided `user_id` and owner hash, and the method returns `Ok(true)`.
    /// - If the character does not exist, it is fetched/created via ESI and then linked to `user_id`,
    ///   and the method returns `Ok(true)`
    ///
    /// # Arguments
    /// - `user_id` (`i32`): The ID of the user to link or transfer the character to. If the user does not exist
    ///   in the database then a database error will be returned (foreign-key constraint)
    /// - `claims` ([`EveJwtClaims`]): JWT claims returned by EVE OAuth2. Contains the character ID
    ///   (`claims.character_id()`) and an owner hash (`claims.owner`) used to determine current ownership.
    ///
    /// # Returns
    /// - `Ok(false)`: No link was created because the character is already linked to the `claims.owner`
    /// - `Ok(true)`: A link was created or the character was transferred to the provided user ID
    /// - `Err(Error::DbErr(_))`: Database error such as a foreign key violation due to invalid `user_id`
    /// - `Err(Error::EsiError(_))`: Error when making an ESI request for character information or parsing
    ///   character ID from claims (e.g. `claims.character_id()`)
    pub async fn link_character(&self, user_id: i32, claims: EveJwtClaims) -> Result<bool, Error> {
        let user_character_repo = UserCharacterRepository::new(&self.db);
        let character_service = CharacterService::new(self.db.clone(), self.esi_client.clone());

        let character_id = claims.character_id()?;

        // If the character exists, check ownership
        if let Some((character, maybe_ownership)) = user_character_repo
            .get_by_character_id(character_id)
            .await?
        {
            if let Some(ownership) = maybe_ownership {
                if ownership.owner_hash == claims.owner && user_id == ownership.user_id {
                    // already linked to this owner -> nothing to do
                    return Ok(false);
                }

                // existing character linked to different owner -> transfer
                self.transfer_character(ownership, user_id).await?;

                return Ok(true);
            }

            // existing character but no owner -> create link
            user_character_repo
                .create(user_id, character.id, claims.owner)
                .await?;

            return Ok(true);
        }

        // character doesn't exist -> create, then link
        let character = character_service.create_character(character_id).await?;
        user_character_repo
            .create(user_id, character.id, claims.owner)
            .await?;

        Ok(true)
    }

    /// Transfers a character from one user to another
    ///
    /// # Behavior
    /// - If this character is the only remaining character for the user,
    ///   the user will then be deleted as they have no way to login.
    pub async fn transfer_character(
        &self,
        ownership_entry: entity::bifrost_user_character::Model,
        new_user_id: i32,
    ) -> Result<bool, Error> {
        let user_repo = UserRepository::new(&self.db);
        let user_character_repo = UserCharacterRepository::new(&self.db);
        let user_service = UserService::new(&self.db, &self.esi_client);

        let (old_user, _) = match user_repo.get(ownership_entry.user_id).await? {
            Some(user) => user,
            None => {
                // This shouldn't occur due to DB foreign key constraints requiring a valid user ID
                return Err(Error::DbErr(sea_orm::DbErr::RecordNotFound(format!(
                    "User not found for user character ownership entry ID {}",
                    ownership_entry.user_id
                ))));
            }
        };

        let ownership_entries = user_character_repo
            .get_many_by_user_id(ownership_entry.user_id)
            .await?;

        user_character_repo
            .update(ownership_entry.id, new_user_id)
            .await?;

        // If this was the last character for the user, delete them
        if ownership_entries.len() == 1 {
            let _ = user_service.delete_user(ownership_entry.user_id).await?;
            return Ok(true);
        }

        // If the user's main character was transferred, change main to oldest linked character
        if ownership_entry.character_id == old_user.main_character_id {
            if let Some(character) = ownership_entries
                .iter()
                .filter(|e| e.character_id != old_user.main_character_id)
                .min_by_key(|e| e.created_at)
            {
                if user_repo
                    .update(old_user.id, character.character_id)
                    .await?
                    .is_none()
                {
                    // This shouldn't occur unless the user were to be deleted while we are trying to update them
                    return Err(Error::DbErr(sea_orm::DbErr::RecordNotFound(format!(
                        "User with ID not found {}",
                        old_user.id
                    ))));
                }
            } else {
                // This shouldn't occur as we delete the user if there is no alternative characters
                return Err(Error::DbErr(sea_orm::DbErr::RecordNotFound(format!(
                    "No alternative character for user {} after removing main character ID {}",
                    old_user.id, old_user.main_character_id
                ))));
            }
        }

        Ok(false)
    }

    pub async fn change_main(&self, user_id: i32, character_id: i64) -> Result<(), Error> {
        let user_repo = UserRepository::new(self.db);
        let user_character_repo = UserCharacterRepository::new(&self.db);

        let character = user_character_repo
            .get_by_character_id(character_id)
            .await?;

        let ownership = match character {
            Some((_, maybe_ownership)) => {
                if let Some(ownership) = maybe_ownership {
                    // Verify the character is owned by this user
                    if ownership.user_id != user_id {
                        tracing::warn!(
                            user_id = %user_id,
                            character_id = %character_id,
                            actual_owner_id = %ownership.user_id,
                            "User attempted to change main to character owned by another user"
                        );

                        return Err(AuthError::CharacterOwnedByAnotherUser.into());
                    }
                    ownership
                } else {
                    tracing::warn!(
                        user_id = %user_id,
                        character_id = %character_id,
                        "User attempted to change main to unowned character"
                    );

                    return Err(AuthError::CharacterNotOwned.into());
                }
            }
            None => {
                tracing::error!(
                    user_id = %user_id,
                    character_id = %character_id,
                    "User attempted to change main to non-existent character"
                );

                return Err(AuthError::CharacterNotFound.into());
            }
        };

        user_repo.update(user_id, ownership.character_id).await?;

        Ok(())
    }
}
