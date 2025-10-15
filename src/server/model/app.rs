use tower_sessions::{MemoryStore, SessionManagerLayer};

#[derive(Clone)]
pub struct AppState {
    pub session: SessionManagerLayer<MemoryStore>,
    pub esi_client: eve_esi::Client,
}
