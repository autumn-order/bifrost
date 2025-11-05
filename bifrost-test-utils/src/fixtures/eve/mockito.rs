use mockito::Mock;

use crate::fixtures::eve::EveFixtures;

impl<'a> EveFixtures<'a> {
    pub fn with_faction_endpoint(&mut self, faction_id: i64, expected_requests: usize) -> Mock {
        let faction = self.with_mock_faction(faction_id);

        self.setup
            .server
            .mock("GET", "/universe/factions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&vec![faction]).unwrap())
            .expect(expected_requests)
            .create()
    }

    pub fn with_alliance_endpoint(
        &mut self,
        alliance_id: i64,
        faction_id: Option<i64>,
        expected_requests: usize,
    ) -> Vec<Mock> {
        let (_, alliance) = self.with_mock_alliance(alliance_id, faction_id);
        let url = format!("/alliances/{}", alliance_id);

        let mut endpoints = Vec::new();

        if let Some(faction_id) = faction_id {
            endpoints.push(self.with_faction_endpoint(faction_id, expected_requests));
        }

        endpoints.push(
            self.setup
                .server
                .mock("GET", url.as_str())
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(serde_json::to_string(&alliance).unwrap())
                .expect(expected_requests)
                .create(),
        );

        endpoints
    }

    pub fn with_corporation_endpoint(
        &mut self,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
        expected_requests: usize,
    ) -> Vec<Mock> {
        let (_, corporation) = self.with_mock_corporation(corporation_id, alliance_id, faction_id);
        let url = format!("/corporations/{}", corporation_id);

        let mut endpoints = Vec::new();

        if let Some(faction_id) = faction_id {
            endpoints.push(self.with_faction_endpoint(faction_id, expected_requests));
        }

        if let Some(alliance_id) = alliance_id {
            endpoints.extend(self.with_alliance_endpoint(alliance_id, None, expected_requests))
        }

        endpoints.push(
            self.setup
                .server
                .mock("GET", url.as_str())
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(serde_json::to_string(&corporation).unwrap())
                .expect(expected_requests)
                .create(),
        );

        endpoints
    }

    pub fn with_character_endpoint(
        &mut self,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
        expected_requests: usize,
    ) -> Vec<Mock> {
        let (_, character) =
            self.with_mock_character(character_id, corporation_id, alliance_id, faction_id);
        let url = format!("/characters/{}", character_id);

        let mut endpoints = Vec::new();

        if let Some(faction_id) = faction_id {
            endpoints.push(self.with_faction_endpoint(faction_id, expected_requests));
        }

        endpoints.push(
            self.setup
                .server
                .mock("GET", url.as_str())
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(serde_json::to_string(&character).unwrap())
                .expect(expected_requests)
                .create(),
        );

        endpoints.extend(self.with_corporation_endpoint(
            corporation_id,
            alliance_id,
            None,
            expected_requests,
        ));

        endpoints
    }
}
