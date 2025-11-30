use eve_esi::model::oauth2::EveJwtClaims;
use oauth2::TokenResponse;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::user::{user_character::UserCharacterRepository, UserRepository},
    error::Error,
    model::db::{CharacterOwnershipModel, EveCharacterModel},
    service::{
        orchestrator::{
            cache::TrackedTransaction, character::CharacterOrchestrator, OrchestrationCache,
        },
        user::user_character::UserCharacterService,
    },
};

/// Represents the current user session state during OAuth callback processing.
enum Session {
    /// No user is currently logged in
    NotLoggedIn,
    /// A user is logged in with the specified user ID
    LoggedIn(i32),
}

/// Represents the database lookup result for a character's existence and ownership status.
enum CharacterRecord {
    /// Character record was not found in database
    NotFound,
    /// Character is in database but not owned by any user
    Unowned { character: EveCharacterModel },
    /// Character is in database and owned by a user
    Owned {
        character: EveCharacterModel,
        ownership: CharacterOwnershipModel,
    },
}

/// Represents the action to take during OAuth callback based on character and session state.
///
/// This enum is determined by the `determine_character_action` function and drives
/// the callback processing logic in `handle_callback`.
pub enum CharacterAction {
    /// Character not in database; fetch from ESI, persist, and link to user
    FetchAndLink {
        /// If None, create a new user; otherwise use existing user ID
        to_user_id: Option<i32>,
        /// EVE Online owner hash from JWT token
        owner_hash: String,
    },
    /// Character is in database but not owned by user; link to a user
    LinkUnownedToUser {
        /// If None, create a new user; otherwise use existing user ID
        to_user_id: Option<i32>,
        /// The existing character record from database
        character: EveCharacterModel,
        /// EVE Online owner hash from JWT token
        owner_hash: String,
    },
    /// Character is owned by another user; transfer it
    TransferOwnership {
        /// If None, create a new user; otherwise use existing user ID
        to_user_id: Option<i32>,
        /// The existing character record from database
        character: EveCharacterModel,
        /// EVE Online owner hash from JWT token
        owner_hash: String,
    },
    /// Same user, owner hash changed (moved EVE accounts but still same user)
    UpdateOwnerHash {
        /// The user ID that owns the character
        user_id: i32,
        /// The existing character record from database
        character: EveCharacterModel,
        /// Updated EVE Online owner hash from JWT token
        owner_hash: String,
    },
    /// No action needed - character is already owned by the current user with matching owner hash
    AlreadyOwned {
        /// The user ID that owns the character
        user_id: i32,
        /// The existing ownership record
        ownership: CharacterOwnershipModel,
    },
}

/// Service for handling OAuth2 callbacks from EVE Online SSO.
///
/// This service orchestrates the authentication flow including token validation,
/// character lookup, ownership management, and user creation/updates.
pub struct CallbackService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CallbackService<'a> {
    /// Creates a new instance of [`CallbackService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Handles the OAuth2 callback after EVE SSO authentication.
    ///
    /// This is the main entry point for processing OAuth2 callbacks from EVE Online SSO.
    /// It orchestrates the entire authentication flow including:
    /// - Validating the authorization code and extracting JWT claims
    /// - Determining the character's ownership status in the database
    /// - Taking appropriate action based on session state and character status
    /// - Optionally updating the user's main character
    ///
    /// The function handles multiple scenarios:
    /// - New character login (fetches from ESI, persists, creates user if needed)
    /// - Existing but unowned character (links to current or new user)
    /// - Character transfer between users (updates ownership, handles main character)
    /// - Owner hash updates (same user, moved EVE accounts)
    /// - Already owned character (no action needed)
    ///
    /// # Arguments
    /// - `authorization_code` - OAuth2 authorization code from EVE SSO redirect
    /// - `user_id` - Optional ID of currently logged-in user (None for new user login)
    /// - `change_main` - Optional flag to set this character as the user's main character
    ///
    /// # Returns
    /// - `Ok(i32)` - The user ID after successful authentication and processing
    /// - `Err(Error::EsiError)` - Failed to fetch or validate OAuth2 token
    /// - `Err(Error::ParseError)` - Failed to parse character ID from JWT claims
    /// - `Err(Error::DbErr)` - Database operation failed
    /// - `Err(Error::AuthError(AuthError::UserNotInDatabase))` - User not found during character transfer
    /// - `Err(Error::AuthError(AuthError::CharacterOwnedByAnotherUser))` - Attempted to set main character owned by different user
    /// - `Err(Error::InternalError)` - Character persistence failed unexpectedly
    pub async fn handle_callback(
        &self,
        authorization_code: &str,
        user_id: Option<i32>,
        change_main: Option<bool>,
    ) -> Result<i32, Error> {
        let claims = self.authenticate_and_get_claims(authorization_code).await?;
        let character_record = self
            .get_character_ownership_status(claims.character_id()?)
            .await?;
        let session = match user_id {
            Some(uid) => Session::LoggedIn(uid),
            None => Session::NotLoggedIn,
        };

        let (user_id, ownership) =
            match determine_character_action(session, character_record, &claims) {
                CharacterAction::FetchAndLink {
                    to_user_id,
                    owner_hash,
                } => {
                    // TODO: this cache will be provided via retry context instead in
                    // a future refactor
                    let mut cache = OrchestrationCache::default();

                    let character_orch = CharacterOrchestrator::new(&self.db, &self.esi_client);

                    let character_id = claims.character_id()?;
                    let fetched_character = character_orch
                        .fetch_character(character_id, &mut cache)
                        .await?;

                    let txn = TrackedTransaction::begin(self.db).await?;

                    let user_repo = UserRepository::new(txn.as_ref());

                    let character = character_orch
                        .persist(&txn, character_id, fetched_character, &mut cache)
                        .await?;

                    let user_id = match to_user_id {
                        Some(uid) => uid,
                        None => user_repo.create(character.id).await?.id,
                    };

                    // Use link_character method to assign newly created character to logged in user
                    let ownership = UserCharacterService::link_character(
                        txn.as_ref(),
                        character.id,
                        user_id,
                        &owner_hash,
                    )
                    .await?;

                    txn.commit().await?;

                    (user_id, ownership)
                }
                CharacterAction::LinkUnownedToUser {
                    to_user_id,
                    character,
                    owner_hash,
                } => {
                    let txn = TrackedTransaction::begin(self.db).await?;

                    let user_repo = UserRepository::new(txn.as_ref());
                    let user_id = match to_user_id {
                        Some(uid) => uid,
                        None => user_repo.create(character.id).await?.id,
                    };

                    // Use link_character method to assign newly created character to logged in user
                    let ownership = UserCharacterService::link_character(
                        txn.as_ref(),
                        character.id,
                        user_id,
                        &owner_hash,
                    )
                    .await?;

                    txn.commit().await?;

                    (user_id, ownership)
                }
                CharacterAction::TransferOwnership {
                    to_user_id,
                    character,
                    owner_hash,
                } => {
                    let txn = TrackedTransaction::begin(self.db).await?;

                    let user_repo = UserRepository::new(txn.as_ref());

                    let user_id = match to_user_id {
                        Some(id) => id,
                        None => user_repo.create(character.id).await?.id,
                    };

                    // Transfer the character from previous user to currently logged in user
                    let ownership = UserCharacterService::transfer_character(
                        txn.as_ref(),
                        character.id,
                        user_id,
                        &owner_hash,
                    )
                    .await?;

                    txn.commit().await?;

                    (user_id, ownership)
                }
                CharacterAction::UpdateOwnerHash {
                    user_id,
                    character,
                    owner_hash,
                } => {
                    let txn = TrackedTransaction::begin(self.db).await?;

                    // Update owner hash via the link_character method
                    // Use link_character method to assign unowned character to logged in user
                    let ownership = UserCharacterService::link_character(
                        txn.as_ref(),
                        character.id,
                        user_id,
                        &owner_hash,
                    )
                    .await?;

                    txn.commit().await?;

                    (user_id, ownership)
                }
                CharacterAction::AlreadyOwned { user_id, ownership } => (user_id, ownership),
            };

        if change_main.unwrap_or(false) {
            let txn = TrackedTransaction::begin(self.db).await?;
            UserCharacterService::set_main_character(txn.as_ref(), user_id, ownership).await?;
            txn.commit().await?;
        }

        Ok(user_id)
    }

    /// Retrieves the ownership status of a character from the database.
    ///
    /// Determines whether a character exists in the database and if so, whether
    /// it is owned by a user or unowned.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID to look up
    ///
    /// # Returns
    /// - `Ok(CharacterRecord::NotFound)` - Character does not exist in database
    /// - `Ok(CharacterRecord::Unowned)` - Character exists but has no owner
    /// - `Ok(CharacterRecord::Owned)` - Character exists and is owned by a user
    /// - `Err(Error::DbErr)` - Database query failed
    async fn get_character_ownership_status(
        &self,
        character_id: i64,
    ) -> Result<CharacterRecord, Error> {
        let user_character_repo = UserCharacterRepository::new(self.db);

        match user_character_repo
            .get_character_with_ownership(character_id)
            .await?
        {
            None => Ok(CharacterRecord::NotFound),
            Some((character, None)) => Ok(CharacterRecord::Unowned { character }),
            Some((character, Some(ownership))) => Ok(CharacterRecord::Owned {
                character,
                ownership,
            }),
        }
    }

    /// Exchanges an authorization code for an access token and validates it.
    ///
    /// Uses the ESI OAuth2 client to exchange the authorization code for tokens,
    /// then validates the access token to extract JWT claims containing character
    /// information and the owner hash.
    ///
    /// # Arguments
    /// - `authorization_code` - OAuth2 authorization code received from EVE SSO callback
    ///
    /// # Returns
    /// - `Ok(EveJwtClaims)` - Validated JWT claims containing character ID and owner hash
    /// - `Err(Error::EsiError)` - Failed to fetch token or validate JWT
    async fn authenticate_and_get_claims(
        &self,
        authorization_code: &str,
    ) -> Result<EveJwtClaims, Error> {
        let token = self
            .esi_client
            .oauth2()
            .get_token(authorization_code)
            .await?;

        let claims = self
            .esi_client
            .oauth2()
            .validate_token(token.access_token().secret().to_string())
            .await?;

        Ok(claims)
    }
}

/// Determines what action to take based on session state, character record, and JWT claims.
///
/// This function implements the business logic for handling different character ownership
/// scenarios during the OAuth callback process. It considers whether the user is logged in,
/// whether the character exists and is owned, and whether the owner hash has changed.
///
/// # Arguments
/// - `session` - Current session state (logged in with user ID, or not logged in)
/// - `record` - Character record status from database lookup
/// - `claims` - Validated JWT claims from EVE SSO containing owner hash
///
/// # Returns
/// - `CharacterAction` - The appropriate action to take for this character
fn determine_character_action(
    session: Session,
    record: CharacterRecord,
    claims: &EveJwtClaims,
) -> CharacterAction {
    match record {
        // Character does not exist in database
        CharacterRecord::NotFound => {
            let to_user_id = match session {
                Session::LoggedIn(uid) => Some(uid),
                Session::NotLoggedIn => None,
            };

            // Fetch character from ESI and link to current user or a new user if not logged in
            CharacterAction::FetchAndLink {
                to_user_id,
                owner_hash: claims.owner.to_string(),
            }
        }
        // Character exists in database but is not owned by any user
        CharacterRecord::Unowned { character } => {
            let to_user_id = match session {
                Session::LoggedIn(uid) => Some(uid),
                Session::NotLoggedIn => None,
            };

            // Link character to current user or a new user if not logged in
            CharacterAction::LinkUnownedToUser {
                to_user_id,
                character,
                owner_hash: claims.owner.to_string(),
            }
        }
        // Character exists in database and is owned by a user
        CharacterRecord::Owned {
            character,
            ownership,
        } => {
            let owner_hash_matches = ownership.owner_hash == claims.owner;
            match session {
                // User is not currently logged in
                Session::NotLoggedIn => {
                    // Character ownership hasn't changed
                    // - login as this user
                    if owner_hash_matches {
                        CharacterAction::AlreadyOwned {
                            user_id: ownership.user_id,
                            ownership,
                        }
                    // Character was transferred to another EVE Online account
                    // - create a new user
                    } else {
                        CharacterAction::TransferOwnership {
                            to_user_id: None, // Create a new user for the character
                            character,
                            owner_hash: claims.owner.to_string(),
                        }
                    }
                }
                Session::LoggedIn(uid) => {
                    match (ownership.user_id == uid, owner_hash_matches) {
                        // Same user, same owner hash - no action needed
                        (true, true) => CharacterAction::AlreadyOwned {
                            user_id: uid,
                            ownership,
                        },
                        // Same user, different owner hash - update hash only
                        (true, false) => CharacterAction::UpdateOwnerHash {
                            user_id: uid,
                            character,
                            owner_hash: claims.owner.to_string(),
                        },
                        // Different user, regardless of hash - transfer ownership
                        (false, _) => CharacterAction::TransferOwnership {
                            to_user_id: Some(uid),
                            character,
                            owner_hash: claims.owner.to_string(),
                        },
                    }
                }
            }
        }
    }
}
