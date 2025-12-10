//! EVE HTTP mock endpoint creation utilities.
//!
//! This module provides methods for creating mock HTTP endpoints that simulate
//! EVE ESI API responses. These endpoints are registered with the mockito server
//! and can verify they were called the expected number of times.

use eve_esi::model::{
    alliance::Alliance,
    character::{Character, CharacterAffiliation},
    corporation::Corporation,
    universe::Faction,
};
use mockito::Mock;

use crate::fixtures::eve::EveFixtures;

impl<'a> EveFixtures<'a> {
    /// Create a mock HTTP endpoint for the factions list.
    ///
    /// Sets up a mock GET endpoint at `/universe/factions` that returns the specified
    /// faction data as JSON. The mock verifies it was called exactly `expected_requests` times.
    ///
    /// # Arguments
    /// - `mock_factions` - Vector of Faction objects to return from the endpoint
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_faction_endpoint(
        &mut self,
        mock_factions: Vec<Faction>,
        expected_requests: usize,
    ) -> Mock {
        self.setup
            .server
            .mock("GET", "/universe/factions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_factions).unwrap())
            .expect(expected_requests)
            .create()
    }

    /// Create a mock HTTP endpoint for alliance data.
    ///
    /// Sets up a mock GET endpoint at `/alliances/{alliance_id}` that returns the specified
    /// alliance data as JSON. The mock verifies it was called exactly `expected_requests` times.
    ///
    /// # Arguments
    /// - `alliance_id` - The alliance ID for the endpoint path
    /// - `mock_alliance` - Alliance object to return from the endpoint
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_alliance_endpoint(
        &mut self,
        alliance_id: i64,
        mock_alliance: Alliance,
        expected_requests: usize,
    ) -> Mock {
        let url = format!("/alliances/{}", alliance_id);

        self.setup
            .server
            .mock("GET", url.as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_alliance).unwrap())
            .expect(expected_requests)
            .create()
    }

    /// Create a mock HTTP endpoint for corporation data.
    ///
    /// Sets up a mock GET endpoint at `/corporations/{corporation_id}` that returns the
    /// specified corporation data as JSON. The mock verifies it was called exactly
    /// `expected_requests` times.
    ///
    /// # Arguments
    /// - `corporation_id` - The corporation ID for the endpoint path
    /// - `mock_corporation` - Corporation object to return from the endpoint
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_corporation_endpoint(
        &mut self,
        corporation_id: i64,
        mock_corporation: Corporation,
        expected_requests: usize,
    ) -> Mock {
        let url = format!("/corporations/{}", corporation_id);

        self.setup
            .server
            .mock("GET", url.as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_corporation).unwrap())
            .expect(expected_requests)
            .create()
    }

    /// Create a mock HTTP endpoint for character data.
    ///
    /// Sets up a mock GET endpoint at `/characters/{character_id}` that returns the
    /// specified character data as JSON. The mock verifies it was called exactly
    /// `expected_requests` times.
    ///
    /// # Arguments
    /// - `character_id` - The character ID for the endpoint path
    /// - `mock_character` - Character object to return from the endpoint
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_character_endpoint(
        &mut self,
        character_id: i64,
        mock_character: Character,
        expected_requests: usize,
    ) -> Mock {
        let url = format!("/characters/{}", character_id);

        self.setup
            .server
            .mock("GET", url.as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_character).unwrap())
            .expect(expected_requests)
            .create()
    }

    /// Create a mock HTTP endpoint for character affiliation data.
    ///
    /// Sets up a mock POST endpoint at `/characters/affiliation` that returns the
    /// specified affiliation data as JSON. The mock verifies it was called exactly
    /// `expected_requests` times.
    ///
    /// # Arguments
    /// - `mock_affiliations` - Vector of CharacterAffiliation objects to return
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_character_affiliation_endpoint(
        &mut self,
        mock_affiliations: Vec<CharacterAffiliation>,
        expected_requests: usize,
    ) -> Mock {
        self.setup
            .server
            .mock("POST", "/characters/affiliation")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_affiliations).unwrap())
            .expect(expected_requests)
            .create()
    }

    /// Create a mock HTTP endpoint that returns an error status code.
    ///
    /// Sets up a mock GET endpoint at `/corporations/{corporation_id}` that returns
    /// the specified error status code. Useful for testing retry logic and error handling.
    ///
    /// # Arguments
    /// - `corporation_id` - The corporation ID for the endpoint path
    /// - `status_code` - HTTP status code to return (e.g., 500, 503, 404)
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_corporation_endpoint_error(
        &mut self,
        corporation_id: i64,
        status_code: usize,
        expected_requests: usize,
    ) -> Mock {
        let url = format!("/corporations/{}", corporation_id);

        self.setup
            .server
            .mock("GET", url.as_str())
            .with_status(status_code)
            .expect(expected_requests)
            .create()
    }

    /// Create a mock HTTP endpoint that returns 304 Not Modified.
    ///
    /// Sets up a mock GET endpoint at `/corporations/{corporation_id}` that returns
    /// 304 Not Modified, indicating the cached data is still current. Used to test
    /// caching behavior.
    ///
    /// # Arguments
    /// - `corporation_id` - The corporation ID for the endpoint path
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_corporation_endpoint_not_modified(
        &mut self,
        corporation_id: i64,
        expected_requests: usize,
    ) -> Mock {
        let url = format!("/corporations/{}", corporation_id);

        self.setup
            .server
            .mock("GET", url.as_str())
            .with_status(304)
            .expect(expected_requests)
            .create()
    }

    /// Create a mock HTTP endpoint for alliance that returns an error status code.
    ///
    /// Sets up a mock GET endpoint at `/alliances/{alliance_id}` that returns
    /// the specified error status code. Useful for testing retry logic and error handling.
    ///
    /// # Arguments
    /// - `alliance_id` - The alliance ID for the endpoint path
    /// - `status_code` - HTTP status code to return (e.g., 500, 503, 404)
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_alliance_endpoint_error(
        &mut self,
        alliance_id: i64,
        status_code: usize,
        expected_requests: usize,
    ) -> Mock {
        let url = format!("/alliances/{}", alliance_id);

        self.setup
            .server
            .mock("GET", url.as_str())
            .with_status(status_code)
            .expect(expected_requests)
            .create()
    }

    /// Create a mock HTTP endpoint that returns 304 Not Modified.
    ///
    /// Sets up a mock GET endpoint at `/alliances/{alliance_id}` that returns
    /// 304 Not Modified, indicating the cached data is still current. Used to test
    /// caching behavior.
    ///
    /// # Arguments
    /// - `alliance_id` - The alliance ID for the endpoint path
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_alliance_endpoint_not_modified(
        &mut self,
        alliance_id: i64,
        expected_requests: usize,
    ) -> Mock {
        let url = format!("/alliances/{}", alliance_id);

        self.setup
            .server
            .mock("GET", url.as_str())
            .with_status(304)
            .expect(expected_requests)
            .create()
    }

    /// Create a mock HTTP endpoint for character that returns an error status code.
    ///
    /// Sets up a mock GET endpoint at `/characters/{character_id}` that returns
    /// the specified error status code. Useful for testing retry logic and error handling.
    ///
    /// # Arguments
    /// - `character_id` - The character ID for the endpoint path
    /// - `status_code` - HTTP status code to return (e.g., 500, 503, 404)
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_character_endpoint_error(
        &mut self,
        character_id: i64,
        status_code: usize,
        expected_requests: usize,
    ) -> Mock {
        let url = format!("/characters/{}", character_id);

        self.setup
            .server
            .mock("GET", url.as_str())
            .with_status(status_code)
            .expect(expected_requests)
            .create()
    }

    /// Create a mock HTTP endpoint that returns 304 Not Modified.
    ///
    /// Sets up a mock GET endpoint at `/characters/{character_id}` that returns
    /// 304 Not Modified, indicating the cached data is still current. Used to test
    /// caching behavior.
    ///
    /// # Arguments
    /// - `character_id` - The character ID for the endpoint path
    /// - `expected_requests` - Number of times this endpoint should be called
    ///
    /// # Returns
    /// - `Mock` - The created mock endpoint that will be automatically verified
    pub fn create_character_endpoint_not_modified(
        &mut self,
        character_id: i64,
        expected_requests: usize,
    ) -> Mock {
        let url = format!("/characters/{}", character_id);

        self.setup
            .server
            .mock("GET", url.as_str())
            .with_status(304)
            .expect(expected_requests)
            .create()
    }
}
