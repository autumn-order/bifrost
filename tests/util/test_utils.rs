//! Test utilities for creating AppState with dummy workers for non-Redis tests

use bifrost::server::{
    model::app::AppState,
    worker::{handler::WorkerJobHandler, Worker},
};
use bifrost_test_utils::TestContext;
use fred::prelude::*;
use sea_orm::DatabaseConnection;

/// Creates a dummy Worker instance for testing purposes.
/// This worker uses a disconnected Redis pool and won't actually process jobs.
pub fn create_dummy_worker(db: DatabaseConnection, esi_client: eve_esi::Client) -> Worker {
    // Create a Redis config that won't actually connect
    let config = Config::default();
    let pool = Pool::new(config, None, None, None, 1).expect("Failed to create dummy Redis pool");

    let handler = WorkerJobHandler::new(db, esi_client);

    // Create worker with minimal concurrent jobs for testing
    Worker::new(1, pool, handler)
}

/// Extension trait for TestContext to create AppState with dummy worker
pub trait TestContextExt {
    fn into_app_state(&self) -> AppState;
}

impl TestContextExt for TestContext {
    fn into_app_state(&self) -> AppState {
        let worker = create_dummy_worker(self.db.clone(), self.esi_client.clone());

        AppState {
            db: self.db.clone(),
            esi_client: self.esi_client.clone(),
            worker,
        }
    }
}
