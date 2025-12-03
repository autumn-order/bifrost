//! Declarative test builder for Phase 1 setup.
//!
//! This module provides the `TestBuilder` API for configuring test environments before execution.
//! The builder pattern allows chaining multiple configuration methods together, with all operations
//! queued and executed during the final `build()` call.

use crate::{error::TestError, TestContext};
use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};
use mockito::Mock;
use sea_orm::{sea_query::TableCreateStatement, EntityTrait, Schema};

/// Builder for declarative test initialization.
///
/// Provides an interface for setting up test environments with database tables,
/// mock fixtures, and HTTP endpoints. Methods can be chained together and finalized
/// with `build()` to create a complete test setup.
pub struct TestBuilder {
    // Tables to create
    tables: Vec<TableCreateStatement>,
    include_user_tables: bool,

    // Database fixtures to insert
    factions: Vec<i64>,
    alliances: Vec<(i64, Option<i64>)>, // (alliance_id, faction_id)
    corporations: Vec<(i64, Option<i64>, Option<i64>)>, // (corp_id, alliance_id, faction_id)
    characters: Vec<(i64, i64, Option<i64>, Option<i64>)>, // (char_id, corp_id, alliance_id, faction_id)
    users_for_characters: Vec<i64>,                        // character_ids to create users for

    // Mock endpoints to create
    mock_builders: Vec<Box<dyn FnOnce(&mut mockito::ServerGuard) -> Mock>>,

    // Pre-configured endpoint shortcuts
    faction_endpoints: Vec<(Vec<Faction>, usize)>, // (factions, expected_requests)
    alliance_endpoints: Vec<(i64, Alliance, usize)>,
    corporation_endpoints: Vec<(i64, Corporation, usize)>,
    character_endpoints: Vec<(i64, Character, usize)>,
    character_affiliation_endpoints:
        Vec<(Vec<eve_esi::model::character::CharacterAffiliation>, usize)>,
    jwt_configs: Vec<(i64, String)>, // (character_id, owner_hash)
}

impl TestBuilder {
    /// Create a new TestBuilder.
    ///
    /// Initializes an empty builder with no tables, fixtures, or mock endpoints configured.
    ///
    /// # Returns
    /// - `TestBuilder` - A new builder instance ready for configuration
    pub fn new() -> Self {
        Self {
            tables: Vec::new(),
            include_user_tables: false,
            factions: Vec::new(),
            alliances: Vec::new(),
            corporations: Vec::new(),
            characters: Vec::new(),
            users_for_characters: Vec::new(),
            mock_builders: Vec::new(),
            faction_endpoints: Vec::new(),
            alliance_endpoints: Vec::new(),
            corporation_endpoints: Vec::new(),
            character_endpoints: Vec::new(),
            character_affiliation_endpoints: Vec::new(),
            jwt_configs: Vec::new(),
        }
    }

    /// Add standard user-related tables to the test database.
    ///
    /// Creates all tables required for user authentication and character management:
    /// EveFaction, EveAlliance, EveCorporation, EveCharacter, BifrostUser, and BifrostUserCharacter.
    ///
    /// # Arguments
    /// - `self` - The builder instance
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_user_tables(mut self) -> Self {
        self.include_user_tables = true;
        self
    }

    /// Add a custom entity table to the test database.
    ///
    /// Generates a CREATE TABLE statement for the entity, which will be executed during `build()`.
    /// Chain multiple calls to add multiple tables.
    ///
    /// # Arguments
    /// - `entity` - Entity type implementing `EntityTrait`
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bifrost_test_utils::TestBuilder;
    /// use entity::prelude::*;
    ///
    /// # async fn example() -> Result<(), bifrost_test_utils::TestError> {
    /// let test = TestBuilder::new()
    ///     .with_table(EveFaction)
    ///     .with_table(EveAlliance)
    ///     .with_table(EveCorporation)
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_table<E: EntityTrait>(mut self, entity: E) -> Self {
        let schema = Schema::new(sea_orm::DbBackend::Sqlite);
        self.tables.push(schema.create_table_from_entity(entity));
        self
    }

    /// Insert mock faction into database.
    ///
    /// Queues a faction fixture to be inserted during `build()`. This only creates
    /// the database record and does not set up any mock HTTP endpoints.
    ///
    /// # Arguments
    /// - `faction_id` - The EVE Online faction ID
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_mock_faction(mut self, faction_id: i64) -> Self {
        self.factions.push(faction_id);
        self
    }

    /// Insert mock alliance into database.
    ///
    /// Queues an alliance fixture to be inserted during `build()`. If a faction_id is
    /// provided, the faction will also be created if it doesn't exist.
    ///
    /// # Arguments
    /// - `alliance_id` - The EVE Online alliance ID
    /// - `faction_id` - Optional faction ID the alliance belongs to
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_mock_alliance(mut self, alliance_id: i64, faction_id: Option<i64>) -> Self {
        self.alliances.push((alliance_id, faction_id));
        self
    }

    /// Insert mock corporation into database.
    ///
    /// Queues a corporation fixture to be inserted during `build()`. Parent entities
    /// (alliance, faction) will be created automatically if specified.
    ///
    /// # Arguments
    /// - `corporation_id` - The EVE Online corporation ID
    /// - `alliance_id` - Optional alliance ID the corporation belongs to
    /// - `faction_id` - Optional faction ID the corporation belongs to
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_mock_corporation(
        mut self,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> Self {
        self.corporations
            .push((corporation_id, alliance_id, faction_id));
        self
    }

    /// Insert mock character into database with full hierarchy.
    ///
    /// Queues a character fixture to be inserted during `build()`. All parent entities
    /// (corporation, alliance, faction) will be created automatically if they don't already exist.
    ///
    /// # Arguments
    /// - `character_id` - The EVE Online character ID
    /// - `corporation_id` - The corporation ID the character belongs to
    /// - `alliance_id` - Optional alliance ID the character belongs to
    /// - `faction_id` - Optional faction ID the character belongs to
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_mock_character(
        mut self,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> Self {
        self.characters
            .push((character_id, corporation_id, alliance_id, faction_id));
        self
    }

    /// Create user and ownership record for a character.
    ///
    /// Queues creation of a BifrostUser and BifrostUserCharacter during `build()`.
    /// The character must be added via `with_mock_character` before calling this method.
    ///
    /// # Arguments
    /// - `character_id` - The character ID to create a user for
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_user_for_character(mut self, character_id: i64) -> Self {
        self.users_for_characters.push(character_id);
        self
    }

    /// Add mock faction endpoint to the test server.
    ///
    /// Creates a mock HTTP endpoint at `/universe/factions` that returns the specified
    /// faction data. The mock will verify it was called exactly `expected_requests` times.
    ///
    /// # Arguments
    /// - `factions` - List of faction objects to return from the endpoint
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_faction_endpoint(
        mut self,
        factions: Vec<Faction>,
        expected_requests: usize,
    ) -> Self {
        self.faction_endpoints.push((factions, expected_requests));
        self
    }

    /// Add mock alliance endpoint to the test server.
    ///
    /// Creates a mock HTTP endpoint at `/alliances/{alliance_id}` that returns the specified
    /// alliance data. The mock will verify it was called exactly `expected_requests` times.
    ///
    /// # Arguments
    /// - `alliance_id` - The alliance ID for the endpoint path
    /// - `alliance` - Alliance object to return from the endpoint
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_alliance_endpoint(
        mut self,
        alliance_id: i64,
        alliance: Alliance,
        expected_requests: usize,
    ) -> Self {
        self.alliance_endpoints
            .push((alliance_id, alliance, expected_requests));
        self
    }

    /// Add mock corporation endpoint to the test server.
    ///
    /// Creates a mock HTTP endpoint at `/corporations/{corporation_id}` that returns the specified
    /// corporation data. The mock will verify it was called exactly `expected_requests` times.
    ///
    /// # Arguments
    /// - `corporation_id` - The corporation ID for the endpoint path
    /// - `corporation` - Corporation object to return from the endpoint
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_corporation_endpoint(
        mut self,
        corporation_id: i64,
        corporation: Corporation,
        expected_requests: usize,
    ) -> Self {
        self.corporation_endpoints
            .push((corporation_id, corporation, expected_requests));
        self
    }

    /// Add mock character endpoint to the test server.
    ///
    /// Creates a mock HTTP endpoint at `/characters/{character_id}` that returns the specified
    /// character data. The mock will verify it was called exactly `expected_requests` times.
    ///
    /// # Arguments
    /// - `character_id` - The character ID for the endpoint path
    /// - `character` - Character object to return from the endpoint
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_character_endpoint(
        mut self,
        character_id: i64,
        character: Character,
        expected_requests: usize,
    ) -> Self {
        self.character_endpoints
            .push((character_id, character, expected_requests));
        self
    }

    /// Add mock character affiliation endpoint to the test server.
    ///
    /// Creates a mock HTTP endpoint at `/characters/affiliation` that returns the specified
    /// affiliation data. The mock will verify it was called exactly `expected_requests` times.
    ///
    /// # Arguments
    /// - `affiliations` - List of character affiliation objects to return
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_character_affiliation_endpoint(
        mut self,
        affiliations: Vec<eve_esi::model::character::CharacterAffiliation>,
        expected_requests: usize,
    ) -> Self {
        self.character_affiliation_endpoints
            .push((affiliations, expected_requests));
        self
    }

    /// Add JWT authentication endpoints to the test server.
    ///
    /// Creates mock HTTP endpoints for both `/oauth/jwks` (JWT keys) and `/v2/oauth/token`
    /// (token exchange) required for EVE SSO authentication flows.
    ///
    /// # Arguments
    /// - `character_id` - Character ID to include in JWT claims
    /// - `owner_hash` - Owner hash to include in JWT claims
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_jwt_endpoints(mut self, character_id: i64, owner_hash: impl Into<String>) -> Self {
        self.jwt_configs.push((character_id, owner_hash.into()));
        self
    }

    /// Add a custom mock endpoint with full control.
    ///
    /// Allows complete customization of mock endpoint behavior by providing direct access
    /// to the mockito ServerGuard. Use this for endpoints not covered by helper methods.
    ///
    /// # Arguments
    /// - `setup` - Closure that receives the mock server and returns a configured Mock
    ///
    /// # Returns
    /// - `Self` - The builder instance for method chaining
    pub fn with_mock_endpoint<F>(mut self, setup: F) -> Self
    where
        F: FnOnce(&mut mockito::ServerGuard) -> Mock + 'static,
    {
        self.mock_builders.push(Box::new(setup));
        self
    }

    /// Build the test setup by creating all configured tables, fixtures, and mock endpoints.
    ///
    /// Executes all queued operations in the following order:
    /// 1. Creates database tables (user tables if specified, then custom tables)
    /// 2. Inserts database fixtures (factions, alliances, corporations, characters, users)
    /// 3. Creates mock HTTP endpoints (ESI endpoints, JWT endpoints, custom endpoints)
    ///
    /// # Returns
    /// - `Ok(TestContext)` - Fully configured test environment ready for use
    /// - `Err(TestError::DbErr)` - Database table creation or fixture insertion failed
    /// - `Err(TestError::EsiError)` - Mock ESI client initialization failed
    pub async fn build(self) -> Result<TestContext, TestError> {
        let mut setup = TestContext::new().await?;

        // 1. Create tables
        let mut all_tables = Vec::new();

        if self.include_user_tables {
            let schema = Schema::new(sea_orm::DbBackend::Sqlite);
            all_tables.extend(vec![
                schema.create_table_from_entity(entity::prelude::EveFaction),
                schema.create_table_from_entity(entity::prelude::EveAlliance),
                schema.create_table_from_entity(entity::prelude::EveCorporation),
                schema.create_table_from_entity(entity::prelude::EveCharacter),
                schema.create_table_from_entity(entity::prelude::BifrostUser),
                schema.create_table_from_entity(entity::prelude::BifrostUserCharacter),
            ]);
        }

        all_tables.extend(self.tables);
        setup.with_tables(all_tables).await?;

        // 2. Insert database fixtures (using existing fixture methods)
        for faction_id in self.factions {
            setup.eve().insert_mock_faction(faction_id).await?;
        }

        for (alliance_id, faction_id) in self.alliances {
            setup
                .eve()
                .insert_mock_alliance(alliance_id, faction_id)
                .await?;
        }

        for (corp_id, alliance_id, faction_id) in self.corporations {
            setup
                .eve()
                .insert_mock_corporation(corp_id, alliance_id, faction_id)
                .await?;
        }

        for (char_id, corp_id, alliance_id, faction_id) in self.characters {
            setup
                .eve()
                .insert_mock_character(char_id, corp_id, alliance_id, faction_id)
                .await?;
        }

        for character_id in self.users_for_characters {
            setup.user().insert_user(character_id as i32).await?;
            setup
                .user()
                .insert_user_character_ownership(character_id as i32, character_id as i32)
                .await?;
        }

        // 3. Create mock endpoints
        // Note: Custom endpoints are created first to allow proper sequential mockito matching
        // when tests need to create multiple mocks for the same path (e.g., error then success)
        let mut mocks = Vec::new();

        for builder in self.mock_builders {
            mocks.push(builder(&mut setup.server));
        }

        for (factions, expected) in self.faction_endpoints {
            mocks.push(setup.eve().create_faction_endpoint(factions, expected));
        }

        for (alliance_id, alliance, expected) in self.alliance_endpoints {
            mocks.push(
                setup
                    .eve()
                    .create_alliance_endpoint(alliance_id, alliance, expected),
            );
        }

        for (corp_id, corporation, expected) in self.corporation_endpoints {
            mocks.push(
                setup
                    .eve()
                    .create_corporation_endpoint(corp_id, corporation, expected),
            );
        }

        for (char_id, character, expected) in self.character_endpoints {
            mocks.push(
                setup
                    .eve()
                    .create_character_endpoint(char_id, character, expected),
            );
        }

        for (affiliations, expected) in self.character_affiliation_endpoints {
            mocks.push(
                setup
                    .eve()
                    .create_character_affiliation_endpoint(affiliations, expected),
            );
        }

        for (char_id, owner_hash) in self.jwt_configs {
            mocks.extend(setup.auth().create_jwt_endpoints(char_id, &owner_hash));
        }

        // Store mocks in setup so they live as long as the test
        setup.mocks = mocks;

        Ok(setup)
    }
}

impl Default for TestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builder_creates_user_tables() {
        let result = TestBuilder::new().with_user_tables().build().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_builder_chains_methods() {
        let result = TestBuilder::new()
            .with_user_tables()
            .with_mock_faction(1)
            .with_mock_alliance(100, Some(1))
            .build()
            .await;
        assert!(result.is_ok());
    }
}
