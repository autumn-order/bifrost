use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::{Mutex, Notify, Semaphore};

use crate::server::model::worker::WorkerJob;
use crate::server::worker::queue::WorkerJobQueue;

/// Persistent job buffer that survives dispatcher panics
///
/// Each dispatcher has its own dedicated buffer (no sharing between dispatchers).
/// Uses Mutex for interior mutability - there's never actual contention since:
/// - During normal operation: Only the owning dispatcher accesses its buffer
/// - During recovery: The dispatcher is already dead, only supervisor accesses it
///
/// This ensures jobs are not lost even with panic = "abort" set.
pub struct DispatcherBuffer {
    jobs: Mutex<VecDeque<WorkerJob>>,
}

impl DispatcherBuffer {
    /// Create a new empty buffer with specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            jobs: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    /// Get mutable access to the job buffer
    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, VecDeque<WorkerJob>> {
        self.jobs.lock().await
    }
}

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
    /// Persistent job buffers that survive dispatcher panics (one per dispatcher)
    pub dispatcher_buffers: Arc<Vec<Arc<DispatcherBuffer>>>,
}

impl DispatcherContext {
    /// Create a new dispatcher context with persistent buffers
    ///
    /// # Arguments
    /// - `dispatcher_count`: Number of dispatchers (determines buffer count)
    /// - `buffer_capacity`: Initial capacity for each dispatcher's buffer
    pub fn new(
        queue: Arc<WorkerJobQueue>,
        db: Arc<DatabaseConnection>,
        esi_client: Arc<eve_esi::Client>,
        semaphore: Arc<Semaphore>,
        shutdown_signal: Arc<Notify>,
        dispatcher_count: usize,
        buffer_capacity: usize,
    ) -> Self {
        // Create persistent buffers for each dispatcher
        let dispatcher_buffers = (0..dispatcher_count)
            .map(|_| Arc::new(DispatcherBuffer::new(buffer_capacity)))
            .collect();

        Self {
            queue,
            db,
            esi_client,
            semaphore,
            shutdown_signal,
            is_shutting_down: Arc::new(AtomicBool::new(false)),
            dispatcher_buffers: Arc::new(dispatcher_buffers),
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

    /// Get a reference to a specific dispatcher's buffer
    pub fn get_buffer(&self, dispatcher_id: usize) -> Option<&Arc<DispatcherBuffer>> {
        self.dispatcher_buffers.get(dispatcher_id)
    }

    /// Drain a dispatcher's buffer and return jobs to the queue
    ///
    /// This is called by the supervisor when a dispatcher dies to prevent job loss.
    /// Jobs are returned with current timestamp for immediate re-processing.
    pub async fn recover_buffer(&self, dispatcher_id: usize) -> Result<usize, String> {
        let buffer = self
            .get_buffer(dispatcher_id)
            .ok_or_else(|| format!("Invalid dispatcher ID: {}", dispatcher_id))?;

        let mut jobs = buffer.lock().await;

        if jobs.is_empty() {
            return Ok(0);
        }

        let buffer_size = jobs.len();
        let now = chrono::Utc::now();
        let jobs_to_return: Vec<_> = jobs.drain(..).map(|job| (job, now)).collect();

        drop(jobs); // Release lock before Redis operation

        match self.queue.push_batch(jobs_to_return).await {
            Ok(added) => Ok(added),
            Err(e) => Err(format!(
                "Failed to return {} jobs to queue: {:?}",
                buffer_size, e
            )),
        }
    }
}
