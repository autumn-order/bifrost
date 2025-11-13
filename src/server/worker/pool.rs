use std::sync::Arc;
use std::time::Duration;

use dioxus_logger::tracing;

use rand::Rng;
use sea_orm::DatabaseConnection;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::server::{
    error::Error,
    model::worker::WorkerJob,
    worker::{new_handler::WorkerJobHandler, queue::WorkerJobQueue},
};

/// Configuration for the worker pool
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    /// Number of concurrent workers
    pub worker_count: usize,
    /// Base delay between polling attempts when queue is empty (in milliseconds)
    ///
    /// Actual delay will be `poll_interval_ms + random(0..poll_interval_ms/2)` to prevent
    /// thundering herd when multiple workers wake simultaneously.
    pub poll_interval_ms: u64,
    /// Maximum number of consecutive errors before a worker backs off
    pub max_consecutive_errors: u32,
    /// Backoff duration after max consecutive errors (in seconds)
    pub error_backoff_seconds: u64,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            worker_count: 4,
            poll_interval_ms: 100,
            max_consecutive_errors: 5,
            error_backoff_seconds: 10,
        }
    }
}

/// Worker pool for processing jobs from the WorkerJobQueue
///
/// The pool manages a configurable number of worker tasks that continuously
/// poll the queue for jobs and process them using the WorkerJobHandler.
///
/// # Features
/// - Configurable number of workers
/// - Graceful shutdown support
/// - Automatic error backoff
/// - Health monitoring
///
/// # Example
/// ```rust,no_run
/// let config = WorkerPoolConfig::default();
/// let pool = WorkerPool::new(config, db, esi_client, queue);
/// pool.start().await?;
///
/// // Later, to stop gracefully
/// pool.stop().await?;
/// ```
pub struct WorkerPool {
    config: WorkerPoolConfig,
    db: Arc<DatabaseConnection>,
    esi_client: Arc<eve_esi::Client>,
    queue: Arc<WorkerJobQueue>,
    workers: Arc<RwLock<Vec<WorkerHandle>>>,
    shutdown_signal: Arc<tokio::sync::Notify>,
}

struct WorkerHandle {
    id: usize,
    handle: JoinHandle<()>,
}

impl WorkerPool {
    /// Create a new worker pool
    pub fn new(
        config: WorkerPoolConfig,
        db: Arc<DatabaseConnection>,
        esi_client: Arc<eve_esi::Client>,
        queue: Arc<WorkerJobQueue>,
    ) -> Self {
        Self {
            config,
            db,
            esi_client,
            queue,
            workers: Arc::new(RwLock::new(Vec::new())),
            shutdown_signal: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Start the worker pool
    ///
    /// Spawns the configured number of worker tasks that will begin processing jobs.
    /// This method is non-blocking and returns immediately after spawning workers.
    pub async fn start(&self) -> Result<(), Error> {
        let mut workers = self.workers.write().await;

        if !workers.is_empty() {
            tracing::warn!("Worker pool is already running");
            return Ok(());
        }

        tracing::info!(
            "Starting worker pool with {} workers",
            self.config.worker_count
        );

        for id in 0..self.config.worker_count {
            let handle = self.spawn_worker(id).await;
            workers.push(WorkerHandle { id, handle });
        }

        tracing::info!("Worker pool started successfully");
        Ok(())
    }

    /// Stop the worker pool gracefully
    ///
    /// Signals all workers to stop and waits for them to complete their current jobs.
    /// This method blocks until all workers have shut down.
    pub async fn stop(&self) -> Result<(), Error> {
        tracing::info!("Shutting down worker pool...");

        // Signal all workers to stop
        self.shutdown_signal.notify_waiters();

        let mut workers = self.workers.write().await;

        // Wait for all workers to finish
        for worker in workers.drain(..) {
            tracing::debug!("Waiting for worker {} to stop", worker.id);
            if let Err(e) = worker.handle.await {
                tracing::error!("Worker {} failed to stop cleanly: {:?}", worker.id, e);
            }
        }

        tracing::info!("Worker pool shut down successfully");
        Ok(())
    }

    /// Check if the worker pool is running
    pub async fn is_running(&self) -> bool {
        let workers = self.workers.read().await;
        !workers.is_empty()
    }

    /// Get the number of active workers
    pub async fn worker_count(&self) -> usize {
        let workers = self.workers.read().await;
        workers.len()
    }

    /// Spawn a single worker task
    async fn spawn_worker(&self, id: usize) -> JoinHandle<()> {
        let db = Arc::clone(&self.db);
        let esi_client = Arc::clone(&self.esi_client);
        let queue = Arc::clone(&self.queue);
        let shutdown_signal = Arc::clone(&self.shutdown_signal);
        let config = self.config.clone();

        tokio::spawn(async move {
            tracing::info!("Worker {} started", id);
            Self::worker_loop(id, config, db, esi_client, queue, shutdown_signal).await;
            tracing::info!("Worker {} stopped", id);
        })
    }

    /// Main worker loop
    ///
    /// Continuously polls the queue for jobs and processes them until shutdown is signaled.
    ///
    /// # Concurrency Optimization
    ///
    /// This implementation uses **jittered backoff** to prevent thundering herd problems:
    /// - When the queue is empty, workers sleep for `poll_interval_ms + random(0..poll_interval_ms/2)`
    /// - This distributes worker wake-up times, reducing Redis contention
    /// - At 100ms base interval, wake-ups are spread across 100-150ms window
    /// - Significantly reduces concurrent Redis calls when queue is empty
    async fn worker_loop(
        id: usize,
        config: WorkerPoolConfig,
        db: Arc<DatabaseConnection>,
        esi_client: Arc<eve_esi::Client>,
        queue: Arc<WorkerJobQueue>,
        shutdown_signal: Arc<tokio::sync::Notify>,
    ) {
        let handler = WorkerJobHandler::new(&db, &esi_client);
        let mut consecutive_errors = 0u32;

        loop {
            // Check for shutdown signal or continue processing
            tokio::select! {
                _ = shutdown_signal.notified() => {
                    tracing::debug!("Worker {} received shutdown signal", id);
                    break;
                }
                result = queue.pop() => {
                    match result {
                        Ok(Some(job)) => {
                            // Reset error counter on successful job retrieval
                            consecutive_errors = 0;

                            tracing::debug!("Worker {} processing job: {:?}", id, job);

                            // Process the job
                            if let Err(e) = Self::process_job(&handler, &job).await {
                                tracing::error!("Worker {} failed to process job: {:?}", id, e);
                            }
                        }
                        Ok(None) => {
                            // Queue is empty, wait before polling again with jittered backoff
                            // to prevent thundering herd when multiple workers wake simultaneously
                            let jitter = rand::rng().random_range(0..config.poll_interval_ms / 2);
                            let delay = config.poll_interval_ms + jitter;
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                        }
                        Err(e) => {
                            consecutive_errors += 1;
                            tracing::error!(
                                "Worker {} error popping job (consecutive errors: {}): {:?}",
                                id,
                                consecutive_errors,
                                e
                            );

                            // Backoff if too many consecutive errors
                            if consecutive_errors >= config.max_consecutive_errors {
                                tracing::warn!(
                                    "Worker {} backing off for {} seconds due to {} consecutive errors",
                                    id,
                                    config.error_backoff_seconds,
                                    consecutive_errors
                                );
                                tokio::time::sleep(Duration::from_secs(config.error_backoff_seconds)).await;
                                consecutive_errors = 0;
                            } else {
                                // Small delay before retrying with jitter
                                let jitter = rand::rng().random_range(0..config.poll_interval_ms / 2);
                                let delay = config.poll_interval_ms + jitter;
                                tokio::time::sleep(Duration::from_millis(delay)).await;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Process a single job
    ///
    /// Delegates to the handler for job execution.
    async fn process_job(handler: &WorkerJobHandler<'_>, job: &WorkerJob) -> Result<(), Error> {
        // Execute the job through the handler
        handler.handle(job).await
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        // Signal shutdown when pool is dropped
        self.shutdown_signal.notify_waiters();
    }
}
