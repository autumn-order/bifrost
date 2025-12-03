//! User and character ownership fixture utilities.
//!
//! This module provides methods for creating user-related test fixtures including
//! BifrostUser records and character ownership relationships. These fixtures are used
//! during test execution (Phase 2) after the test environment has been set up.

use crate::model::{CharacterOwnershipModel, EveCharacterModel, UserModel};
use chrono::Utc;
use sea_orm::{ActiveValue, EntityTrait};

use crate::{error::TestError, TestContext};

impl TestContext {
    /// Access user fixture helper methods.
    ///
    /// Returns a UserFixtures instance for creating and managing user-related
    /// test data during test execution.
    ///
    /// # Arguments
    /// - `self` - Mutable reference to TestContext
    ///
    /// # Returns
    /// - `UserFixtures` - Helper for user fixture operations
    pub fn user<'a>(&'a mut self) -> UserFixtures<'a> {
        UserFixtures { setup: self }
    }
}

/// Helper struct for user-related fixture operations.
///
/// Provides methods for inserting users, character ownership records, and complete
/// user-character relationships into the test database. Access via `TestContext::user()`.
pub struct UserFixtures<'a> {
    setup: &'a mut TestContext,
}

impl<'a> UserFixtures<'a> {
    /// Insert a user into the database with a main character.
    ///
    /// Creates a BifrostUser record with the specified character as the main character.
    /// The user is assigned the current timestamp as creation time.
    ///
    /// # Arguments
    /// - `character_id` - The character ID to set as the user's main character
    ///
    /// # Returns
    /// - `Ok(UserModel)` - The created user record
    /// - `Err(TestError::DbErr)` - Database insert operation failed
    pub async fn insert_user(&self, character_id: i32) -> Result<UserModel, TestError> {
        Ok(
            entity::prelude::BifrostUser::insert(entity::bifrost_user::ActiveModel {
                main_character_id: ActiveValue::Set(character_id),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            })
            .exec_with_returning(&self.setup.db)
            .await?,
        )
    }

    /// Insert a character ownership record linking a user to a character.
    ///
    /// Creates a BifrostUserCharacter record establishing ownership between a user
    /// and a character. Uses a default owner hash value of "owner_hash".
    ///
    /// # Arguments
    /// - `user_id` - The ID of the user who owns the character
    /// - `character_id` - The ID of the character being owned
    ///
    /// # Returns
    /// - `Ok(CharacterOwnershipModel)` - The created ownership record
    /// - `Err(TestError::DbErr)` - Database insert operation failed
    pub async fn insert_user_character_ownership(
        &self,
        user_id: i32,
        character_id: i32,
    ) -> Result<CharacterOwnershipModel, TestError> {
        Ok(entity::prelude::BifrostUserCharacter::insert(
            entity::bifrost_user_character::ActiveModel {
                user_id: ActiveValue::Set(user_id),
                character_id: ActiveValue::Set(character_id),
                owner_hash: ActiveValue::Set("owner_hash".to_string()),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                ..Default::default()
            },
        )
        .exec_with_returning(&self.setup.db)
        .await?)
    }

    /// Create a complete user with character and full EVE hierarchy.
    ///
    /// Creates a character record with full corporate/alliance/faction hierarchy,
    /// then creates a user with that character as the main character, and establishes
    /// the ownership relationship. This is a convenience method that combines multiple
    /// fixture operations into one call.
    ///
    /// # Arguments
    /// - `character_id` - The EVE Online character ID to create
    /// - `corporation_id` - The corporation ID the character belongs to
    /// - `alliance_id` - Optional alliance ID the character belongs to
    /// - `faction_id` - Optional faction ID the character belongs to
    ///
    /// # Returns
    /// - `Ok((UserModel, CharacterOwnershipModel, EveCharacterModel))` - Tuple of created records
    /// - `Err(TestError::DbErr)` - Database insert operation failed
    pub async fn insert_user_with_mock_character(
        &mut self,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> Result<(UserModel, CharacterOwnershipModel, EveCharacterModel), TestError> {
        let character_model = self
            .setup
            .eve()
            .insert_mock_character(character_id, corporation_id, alliance_id, faction_id)
            .await?;

        let user_model = self.insert_user(character_model.id).await?;

        let user_character_model = self
            .insert_user_character_ownership(user_model.id, character_model.id)
            .await?;

        Ok((user_model, user_character_model, character_model))
    }

    /// Add a character to an existing user.
    ///
    /// Creates a character record with full corporate/alliance/faction hierarchy,
    /// then establishes ownership between the specified user and the new character.
    /// Useful for testing multi-character scenarios.
    ///
    /// # Arguments
    /// - `user_id` - The existing user ID to link the character to
    /// - `character_id` - The EVE Online character ID to create
    /// - `corporation_id` - The corporation ID the character belongs to
    /// - `alliance_id` - Optional alliance ID the character belongs to
    /// - `faction_id` - Optional faction ID the character belongs to
    ///
    /// # Returns
    /// - `Ok((CharacterOwnershipModel, EveCharacterModel))` - Tuple of ownership and character records
    /// - `Err(TestError::DbErr)` - Database insert operation failed
    pub async fn insert_mock_character_for_user(
        &mut self,
        user_id: i32,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> Result<(CharacterOwnershipModel, EveCharacterModel), TestError> {
        let character_model = self
            .setup
            .eve()
            .insert_mock_character(character_id, corporation_id, alliance_id, faction_id)
            .await?;

        let user_character_model = self
            .insert_user_character_ownership(user_id, character_model.id)
            .await?;

        Ok((user_character_model, character_model))
    }
}
