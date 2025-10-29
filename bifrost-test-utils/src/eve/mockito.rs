use mockito::Mock;

use crate::TestSetup;

impl TestSetup {
    pub fn with_faction_endpoint(&mut self, faction_id: i64, expected_requests: usize) -> Mock {
        let faction = self.with_mock_faction(faction_id);

        self.server
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
    ) -> Mock {
        let (_, alliance) = self.with_mock_alliance(alliance_id, faction_id);
        let url = format!("/alliances/{}", alliance_id);

        self.server
            .mock("GET", url.as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&alliance).unwrap())
            .expect(expected_requests)
            .create()
    }
}
