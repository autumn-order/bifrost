use eve_esi::model::corporation::Corporation;
use mockito::{Mock, ServerGuard};

/// Create a mock ESI corporation endpoint
///
/// Set the url to /corporations/1 to fetch corporation with id 1
pub fn mock_corporation_endpoint(
    server: &mut ServerGuard,
    url: &'static str,
    corporation: Corporation,
    expected_requests: usize,
) -> Mock {
    server
        .mock("GET", url)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&corporation).unwrap())
        .expect(expected_requests)
        .create()
}
