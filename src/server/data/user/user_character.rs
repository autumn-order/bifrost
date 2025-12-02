//! User-character ownership repository.
//!
//! This module provides the `UserCharacterRepository` for managing the ownership links
//! between user accounts and EVE Online characters. It handles character ownership tracking,
//! querying ownership status, and retrieving characters with their affiliations.

use crate::server::model::db::{
    CharacterOwnershipModel, EveAllianceModel, EveCharacterModel, EveCorporationModel,
};
use chrono::Utc;
use dioxus_logger::tracing;
use migration::OnConflict;
use sea_orm::{ActiveValue, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};

/// Repository for managing user-character ownership relationships in the database.
///
/// Provides operations for linking characters to users, querying ownership status,
/// and retrieving character information with ownership and corporation/alliance details.
pub struct UserCharacterRepository<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> UserCharacterRepository<'a, C> {
    /// Creates a new instance of UserCharacterRepository.
    ///
    /// Constructs a repository for managing user-character relationships in the database.
    ///
    /// # Arguments
    /// - `db` - Database connection reference
    ///
    /// # Returns
    /// - `UserCharacterRepository` - New repository instance
    pub fn new(db: &'a C) -> Self {
        Self { db }
    }

    /// Inserts or updates a user-character ownership record.
    ///
    /// Creates a new ownership link between a user and character, or updates the existing
    /// record if one already exists for the given character_id. On conflict, updates the
    /// user_id, owner_hash, and updated_at fields.
    ///
    /// # Arguments
    /// - `character_id` - Internal database ID of the character record
    /// - `user_id` - ID of the user who owns this character
    /// - `owner_hash` - EVE SSO owner hash for ownership verification
    ///
    /// # Returns
    /// - `Ok(BifrostUserCharacter)` - The created or updated ownership record
    /// - `Err(DbErr)` - Database operation failed
    pub async fn upsert(
        &self,
        character_id: i32,
        user_id: i32,
        owner_hash: String,
    ) -> Result<CharacterOwnershipModel, DbErr> {
        entity::prelude::BifrostUserCharacter::insert(entity::bifrost_user_character::ActiveModel {
            user_id: ActiveValue::Set(user_id),
            character_id: ActiveValue::Set(character_id),
            owner_hash: ActiveValue::Set(owner_hash),
            created_at: ActiveValue::Set(Utc::now().naive_utc()),
            updated_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(entity::bifrost_user_character::Column::CharacterId)
                .update_columns([
                    entity::bifrost_user_character::Column::UserId,
                    entity::bifrost_user_character::Column::OwnerHash,
                    entity::bifrost_user_character::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec_with_returning(self.db)
        .await
    }

    /// Retrieves the character ownership record by the character's internal database ID.
    ///
    /// This function fetches the ownership record (user-character link) from the
    /// `bifrost_user_character` table using the character's internal record ID.
    /// Note: This uses the internal database ID (`id` column), not the EVE character ID.
    ///
    /// # Arguments
    /// - `character_record_id` - Internal database ID for the character entry
    ///
    /// # Returns
    /// - `Ok(Some(BifrostUserCharacter))` - Ownership record found for this character
    /// - `Ok(None)` - Character exists but has no ownership record (unowned character)
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_ownership_by_character_id(
        &self,
        character_record_id: i32,
    ) -> Result<Option<CharacterOwnershipModel>, DbErr> {
        entity::prelude::BifrostUserCharacter::find_by_id(character_record_id)
            .one(self.db)
            .await
    }

    /// Retrieves a character and its ownership status by EVE Online character ID.
    ///
    /// This function fetches the character record from the `eve_character` table along
    /// with its optional ownership record from the `bifrost_user_character` table.
    /// This is useful for determining if a character exists and whether it's owned by a user.
    ///
    /// # Arguments
    /// - `eve_character_id` - EVE Online character ID
    ///
    /// # Returns
    /// - `Ok(Some((character, Some(ownership))))` - Character exists and is owned by a user
    /// - `Ok(Some((character, None)))` - Character exists but is not owned by any user
    /// - `Ok(None)` - Character does not exist in the database
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_character_with_ownership(
        &self,
        eve_character_id: i64,
    ) -> Result<Option<(EveCharacterModel, Option<CharacterOwnershipModel>)>, DbErr> {
        entity::prelude::EveCharacter::find()
            .filter(entity::eve_character::Column::CharacterId.eq(eve_character_id))
            .find_also_related(entity::bifrost_user_character::Entity)
            .one(self.db)
            .await
    }

    /// Retrieves all character ownership records for a user.
    ///
    /// Fetches all user-character ownership links for the specified user ID from
    /// the bifrost_user_character table. Returns an empty vector if the user has no characters.
    ///
    /// # Arguments
    /// - `user_id` - ID of the user whose character ownerships to retrieve
    ///
    /// # Returns
    /// - `Ok(Vec<BifrostUserCharacter>)` - List of ownership records (empty if user has no characters)
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_ownerships_by_user_id(
        &self,
        user_id: i32,
    ) -> Result<Vec<CharacterOwnershipModel>, DbErr> {
        entity::prelude::BifrostUserCharacter::find()
            .filter(entity::bifrost_user_character::Column::UserId.eq(user_id))
            .all(self.db)
            .await
    }

    /// Retrieves complete character information for all characters owned by a user.
    ///
    /// Fetches all characters owned by the specified user along with their corporation
    /// and optional alliance details. Characters are joined with their corporations and
    /// alliances. Characters missing corporation data are logged as warnings and excluded
    /// from results.
    ///
    /// # Arguments
    /// - `user_id` - ID of the user whose characters to retrieve
    ///
    /// # Returns
    /// - `Ok(Vec<(EveCharacter, EveCorporation, Option<EveAlliance>)>)` - List of characters with corp/alliance info (empty if user has no characters)
    /// - `Err(DbErr)` - Database query failed
    pub async fn get_owned_characters_by_user_id(
        &self,
        user_id: i32,
    ) -> Result<
        Vec<(
            EveCharacterModel,
            EveCorporationModel,
            Option<EveAllianceModel>,
        )>,
        DbErr,
    > {
        let user_characters: Vec<(CharacterOwnershipModel, Option<EveCharacterModel>)> =
            entity::prelude::BifrostUserCharacter::find()
                .filter(entity::bifrost_user_character::Column::UserId.eq(user_id))
                .find_also_related(entity::prelude::EveCharacter)
                .all(self.db)
                .await?;

        if user_characters.is_empty() {
            return Ok(Vec::new());
        }

        let corporation_ids: Vec<i32> = user_characters
            .iter()
            .filter_map(|(_, eve_char)| eve_char.as_ref().map(|c| c.corporation_id))
            .collect();

        let corporations: std::collections::HashMap<
            i32,
            (EveCorporationModel, Option<EveAllianceModel>),
        > = entity::prelude::EveCorporation::find()
            .filter(entity::eve_corporation::Column::Id.is_in(corporation_ids))
            .find_also_related(entity::prelude::EveAlliance)
            .all(self.db)
            .await?
            .into_iter()
            .map(|(corp, alliance)| (corp.id, (corp, alliance)))
            .collect();

        // Build the result by matching corporations (with alliances) to characters
        // Filter out entries without corporations
        let result = user_characters
            .into_iter()
            .filter_map(|(_, eve_char)| {
                match eve_char {
                    Some(character) => {
                        match corporations.get(&character.corporation_id) {
                            Some((corporation, alliance)) => {
                                Some((character, corporation.clone(), alliance.clone()))
                            }
                            None => {
                                tracing::warn!(
                                    character_id = character.id,
                                    character_name = %character.name,
                                    corporation_id = character.corporation_id,
                                    "Failed to find related corporation for character in database - skipping character from results"
                                );
                                None
                            }
                        }
                    }
                    None => None,
                }
            })
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {

    mod get_by_character_id {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::user::user_character::UserCharacterRepository;

        // Expect Some when character & character ownership entry is found
        #[tokio::test]
        async fn finds_character_with_ownership() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (_, _, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_character_with_ownership(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let maybe_character = result.unwrap();
            assert!(maybe_character.is_some());
            let (_, maybe_owner) = maybe_character.unwrap();
            assert!(maybe_owner.is_some());

            Ok(())
        }

        // Expect Some when character entry is found but no character ownership entry
        #[tokio::test]
        async fn finds_character_without_ownership() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_character_with_ownership(character_model.character_id)
                .await;

            assert!(result.is_ok());
            let maybe_character = result.unwrap();
            assert!(maybe_character.is_some());
            let (_, maybe_owner) = maybe_character.unwrap();
            assert!(maybe_owner.is_none());

            Ok(())
        }

        // Expect None when character is not found
        #[tokio::test]
        async fn returns_none_for_nonexistent_character() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;

            let nonexistent_character_id = 1;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_character_with_ownership(nonexistent_character_id)
                .await;

            assert!(result.is_ok());
            let maybe_character = result.unwrap();
            assert!(maybe_character.is_none());

            Ok(())
        }

        // Expect Error when required database tables are not present
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            // Use test setup that does not create required tables, causing a database error
            let test = test_setup_with_tables!()?;

            let character_id = 1;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_character_with_ownership(character_id)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod get_ownerships_by_user_id {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::user::user_character::UserCharacterRepository;

        /// Expect Ok with 2 owned character entries
        #[tokio::test]
        async fn returns_multiple_characters() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;
            let (_, _) = test
                .user()
                .insert_mock_character_for_user(user_model.id, 2, 1, None, None)
                .await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .get_ownerships_by_user_id(user_model.id)
                .await;

            assert!(result.is_ok());
            let ownership_entries = result.unwrap();
            assert_eq!(ownership_entries.len(), 2);

            Ok(())
        }

        /// Expect Ok with only 1 owned character entry
        #[tokio::test]
        async fn returns_single_character() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .get_ownerships_by_user_id(user_model.id)
                .await;

            assert!(result.is_ok());
            let ownership_entries = result.unwrap();
            assert_eq!(ownership_entries.len(), 1);

            Ok(())
        }

        /// Expect Ok with empty Vec due to no owned characters
        #[tokio::test]
        async fn returns_empty_for_no_characters() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
            // Character is set as main but it is not actually owned due to no ownership entry
            let user_model = test.user().insert_user(character_model.id).await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .get_ownerships_by_user_id(user_model.id)
                .await;

            assert!(result.is_ok());
            let ownership_entries = result.unwrap();
            assert!(ownership_entries.is_empty());

            Ok(())
        }

        /// Expect database error when required tables aren't present
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            // Use test setup that doesn't create required tables, causing an error
            let test = test_setup_with_tables!()?;

            let user_id = 1;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_ownerships_by_user_id(user_id)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }

    mod get_owned_characters_by_user_id {
        use bifrost_test_utils::prelude::*;

        use crate::server::data::user::user_character::UserCharacterRepository;

        /// Expect Ok with Vec length of 1 when requesting valid user ID
        /// Validates that corporation is present and alliance can be None
        #[tokio::test]
        async fn returns_owned_characters_for_user() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, character_model) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .get_owned_characters_by_user_id(user_model.id)
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 1);

            let (character, corporation, alliance) = &characters[0];
            assert_eq!(character.id, character_model.id);
            assert_eq!(character.name, character_model.name);
            // Corporation should always be present
            assert_eq!(corporation.corporation_id, 1);
            // Alliance should be None since we didn't create one
            assert!(alliance.is_none());

            Ok(())
        }

        /// Expect Ok with Vec containing character with alliance
        #[tokio::test]
        async fn returns_owned_characters_with_alliance() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;

            let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
            let corporation_model = test.eve().insert_mock_corporation(1, Some(1), None).await?;
            let character_model = test
                .eve()
                .insert_mock_character(
                    1,
                    corporation_model.corporation_id,
                    Some(alliance_model.alliance_id),
                    None,
                )
                .await?;

            // Create user
            let user_model = test.user().insert_user(character_model.id).await?;

            // Create ownership entry
            test.user()
                .insert_user_character_ownership(user_model.id, character_model.id)
                .await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .get_owned_characters_by_user_id(user_model.id)
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 1);

            // Validate alliance is present
            let (character, corporation, alliance) = &characters[0];
            assert_eq!(character.id, character_model.id);
            assert_eq!(corporation.corporation_id, 1);
            // Alliance should be present
            assert!(alliance.is_some());
            assert_eq!(alliance.as_ref().unwrap().id, alliance_model.id);

            Ok(())
        }

        /// Expect Ok with multiple characters, validating all have corporations
        #[tokio::test]
        async fn returns_multiple_owned_characters() -> Result<(), TestError> {
            let mut test = test_setup_with_user_tables!()?;
            let (user_model, _, _) = test
                .user()
                .insert_user_with_mock_character(1, 1, None, None)
                .await?;
            test.user()
                .insert_mock_character_for_user(user_model.id, 2, 2, Some(1), None)
                .await?;

            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .get_owned_characters_by_user_id(user_model.id)
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 2);

            // Validate both characters have corporations
            for (character, corporation, _) in &characters {
                assert!(character.id > 0);
                assert!(corporation.id > 0);
            }

            Ok(())
        }

        /// Expect Ok with empty Vec when requesting a nonexistent user ID
        #[tokio::test]
        async fn returns_empty_for_nonexistent_user() -> Result<(), TestError> {
            let test = test_setup_with_user_tables!()?;

            let nonexistent_user_id = 1;
            let user_character_repository = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repository
                .get_owned_characters_by_user_id(nonexistent_user_id)
                .await;

            assert!(result.is_ok());
            let characters = result.unwrap();
            assert_eq!(characters.len(), 0);

            Ok(())
        }

        /// Expect Error when required database tables do not exist
        #[tokio::test]
        async fn fails_when_tables_missing() -> Result<(), TestError> {
            // Use test setup that doesn't setup required tables, causing a database error
            let test = test_setup_with_tables!()?;

            let nonexistent_user_id = 1;
            let user_character_repo = UserCharacterRepository::new(&test.state.db);
            let result = user_character_repo
                .get_owned_characters_by_user_id(nonexistent_user_id)
                .await;

            assert!(result.is_err());

            Ok(())
        }
    }
}
