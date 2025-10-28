use eve_esi::model::universe::Faction;
use mockito::{Mock, ServerGuard};

/// Create a mock ESI faction endpoint
pub fn mock_faction_endpoint(
    server: &mut ServerGuard,
    factions: Vec<Faction>,
    expected_requests: usize,
) -> Mock {
    server
        .mock("GET", "/universe/factions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&factions).unwrap())
        .expect(expected_requests)
        .create()
}
