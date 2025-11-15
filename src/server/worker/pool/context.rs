use std::sync::atomic::{AtomicBool, Ordering};
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
    pub is_shutting_down: Arc<AtomicBool>,
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
            is_shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the number of available semaphore permits
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Check if shutdown has been initiated
    pub fn is_shutting_down(&self) -> bool {
        self.is_shutting_down.load(Ordering::Relaxed)
    }

    /// Mark shutdown as initiated
    pub fn set_shutting_down(&self) {
        self.is_shutting_down.store(true, Ordering::Relaxed);
    }
}
