use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::{Notify, Semaphore};

use crate::server::worker::queue::WorkerJobQueue;

/// Shared context for job dispatching and execution
///
/// Bundles all the Arc'd resources needed by dispatchers and job handlers,
/// reducing the number of parameters passed through function calls.
#[derive(Clone)]
pub struct DispatcherContext {
    pub queue: Arc<WorkerJobQueue>,
    pub db: Arc<DatabaseConnection>,
    pub esi_client: Arc<eve_esi::Client>,
    pub semaphore: Arc<Semaphore>,
    pub shutdown_signal: Arc<Notify>,
}

impl DispatcherContext {
    /// Create a new dispatcher context
    pub fn new(
        queue: Arc<WorkerJobQueue>,
        db: Arc<DatabaseConnection>,
        esi_client: Arc<eve_esi::Client>,
        semaphore: Arc<Semaphore>,
        shutdown_signal: Arc<Notify>,
    ) -> Self {
        Self {
            queue,
            db,
            esi_client,
            semaphore,
            shutdown_signal,
        }
    }

    /// Get the number of available semaphore permits
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}
