use std::sync::Arc;

use tower_sessions::{MemoryStore, Session};

/// Creates a [`Session`] instance used for session-related tests
pub fn session_test_setup() -> Session {
    let store = Arc::new(MemoryStore::default());
    Session::new(None, store, None)
}
