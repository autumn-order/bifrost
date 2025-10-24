use eve_esi::model::character::Character;
use mockito::{Mock, ServerGuard};

/// Create a mock ESI character endpoint
///
/// Set the url to /characters/1 to fetch corporation with id 1
pub fn mock_character_endpoint(
    server: &mut ServerGuard,
    url: &'static str,
    character: Character,
    expected_requests: usize,
) -> Mock {
    server
        .mock("GET", url)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&character).unwrap())
        .expect(expected_requests)
        .create()
}
