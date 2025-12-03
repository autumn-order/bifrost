use eve_esi::model::{
    alliance::Alliance,
    character::{Character, CharacterAffiliation},
    corporation::Corporation,
    universe::Faction,
};
use mockito::Mock;

use crate::fixtures::eve::EveFixtures;

impl<'a> EveFixtures<'a> {
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
}
