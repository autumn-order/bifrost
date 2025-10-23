use eve_esi::model::alliance::Alliance;
use mockito::{Mock, ServerGuard};

/// Create a mock ESI alliance endpoint
///
/// Set the url to /alliance/1 to fetch alliance with id 1
pub fn mock_alliance_endpoint(
    server: &mut ServerGuard,
    url: &'static str,
    alliance: Alliance,
    expected_requests: usize,
) -> Mock {
    server
        .mock("GET", url)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&alliance).unwrap())
        .expect(expected_requests)
        .create()
}
