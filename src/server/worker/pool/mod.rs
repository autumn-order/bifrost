mod config;

pub use config::WorkerPoolConfig;

use std::sync::Arc;
use std::time::Duration;

use dioxus_logger::tracing;
use tokio::sync::{Notify, RwLock, Semaphore};
use tokio::task::JoinHandle;

use crate::server::worker::handler::WorkerJobHandler;
use crate::server::{error::Error, worker::queue::WorkerQueue};

/// Worker pool for processing jobs from the WorkerJobQueue
///
/// Simple implementation that spawns dispatcher tasks to poll Redis and
/// process jobs with semaphore-based concurrency control.
#[derive(Clone)]
pub struct WorkerPool {
    inner: Arc<WorkerPoolRef>,
}

#[derive(Clone)]
pub struct WorkerPoolRef {
    config: WorkerPoolConfig,
    queue: WorkerQueue,
    handler: Arc<WorkerJobHandler>,
    semaphore: Arc<Semaphore>,
    shutdown: Arc<Notify>,
    dispatcher_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
}

impl WorkerPool {
    /// Create a new worker pool
    ///
    /// # Arguments
    /// - `config`: Configuration including max concurrent jobs and dispatcher settings
    /// - `db`: Database connection pool (shared across all job tasks)
    /// - `esi_client`: EVE ESI API client (shared across all job tasks)
    /// - `queue`: Redis-backed job queue
    pub fn new(config: WorkerPoolConfig, queue: WorkerQueue, handler: WorkerJobHandler) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_jobs));
        let shutdown = Arc::new(Notify::new());

        Self {
            inner: Arc::new(WorkerPoolRef {
                config,
                handler: Arc::new(handler),
                queue: queue,
                semaphore,
                shutdown,
                dispatcher_handles: Arc::new(RwLock::new(Vec::new())),
            }),
        }
    }

    /// Start the worker pool
    ///
    /// Spawns the configured number of dispatcher tasks that poll Redis for jobs
    /// and spawn tasks to process them, with the semaphore controlling maximum
    /// concurrency.
    ///
    /// This method is non-blocking and returns immediately after spawning dispatchers.
    pub async fn start(&self) -> Result<(), Error> {
        let mut handles = self.inner.dispatcher_handles.write().await;

        if !handles.is_empty() {
            tracing::warn!("Worker pool is already running");
            return Ok(());
        }

        tracing::info!(
            "Starting worker pool with {} dispatcher(s) (max {} concurrent jobs)",
            self.inner.config.dispatcher_count,
            self.inner.config.max_concurrent_jobs
        );

        // Start the job queue cleanup task
        self.inner.queue.start_cleanup().await;

        // Spawn all dispatcher tasks
        for id in 0..self.inner.config.dispatcher_count {
            let handle = self.spawn_dispatcher(id);
            handles.push(handle);
        }

        tracing::info!(
            "Worker pool started successfully ({} dispatcher(s) active)",
            self.inner.config.dispatcher_count
        );

        Ok(())
    }

    /// Spawn a single dispatcher task
    fn spawn_dispatcher(&self, id: usize) -> JoinHandle<()> {
        let config = self.inner.config.clone();
        let queue = self.inner.queue.clone();
        let handler = Arc::clone(&self.inner.handler);
        let semaphore = Arc::clone(&self.inner.semaphore);
        let shutdown = Arc::clone(&self.inner.shutdown);

        tokio::spawn(async move {
            tracing::info!("Dispatcher {} started", id);

            loop {
                tokio::select! {
                    // Biased select ensures shutdown signal is prioritized
                    // over processing new jobs, enabling faster shutdown.
                    biased;

                    _ = shutdown.notified() => {
                        tracing::debug!("Dispatcher {} received shutdown signal", id);
                        break;
                    }

                    _ = Self::process_jobs(
                        id,
                        &config,
                        &queue,
                        &handler,
                        &semaphore,
                    ) => {
                        // Continue to next iteration
                    }
                }
            }

            tracing::info!("Dispatcher {} stopped", id);
        })
    }

    /// Process jobs from the queue
    ///
    /// Polls Redis for a job and spawns a task to process it if one is available.
    /// Sleeps if the queue is empty or on error.
    async fn process_jobs(
        dispatcher_id: usize,
        config: &WorkerPoolConfig,
        queue: &WorkerQueue,
        handler: &Arc<WorkerJobHandler>,
        semaphore: &Arc<Semaphore>,
    ) {
        match queue.pop().await {
            Ok(Some(job)) => {
                // Try to acquire a permit (blocks if at capacity)
                match semaphore.clone().acquire_owned().await {
                    Ok(permit) => {
                        // Clone Arc references for the spawned task
                        let handler = Arc::clone(handler);
                        let timeout = config.job_timeout();

                        // Spawn task to execute the job
                        tokio::spawn(async move {
                            Self::execute_job(job, handler, timeout, permit).await;
                        });
                    }
                    Err(_) => {
                        // Semaphore closed (shutting down), push job back
                        let _ = queue.push(job).await;
                        tracing::debug!(
                            "Dispatcher {} semaphore closed, returned job to queue",
                            dispatcher_id
                        );
                    }
                }
            }
            Ok(None) => {
                // Queue is empty, sleep before next poll
                tokio::time::sleep(config.poll_interval()).await;
            }
            Err(e) => {
                // Error fetching from queue, log and backoff
                tracing::error!("Dispatcher {} queue error: {:?}", dispatcher_id, e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    /// Execute a job with timeout
    ///
    /// Wraps job execution with timeout to prevent hung jobs.
    /// The semaphore permit is held until this function completes, limiting concurrency.
    async fn execute_job(
        job: crate::server::model::worker::WorkerJob,
        handler: Arc<WorkerJobHandler>,
        timeout: Duration,
        _permit: tokio::sync::OwnedSemaphorePermit,
    ) {
        // Execute job with timeout
        let result = tokio::time::timeout(timeout, handler.handle(&job)).await;

        match result {
            Ok(Ok(())) => {
                // Job completed successfully
                tracing::debug!("Job completed: {}", job);
            }
            Ok(Err(e)) => {
                tracing::error!("Job failed: {}, error: {:?}", job, e);
            }
            Err(_) => {
                tracing::error!("Job timed out after {} seconds: {}", timeout.as_secs(), job);
            }
        }

        // Permit automatically dropped here, releasing semaphore slot
    }

    /// Stop the worker pool gracefully
    ///
    /// Signals all dispatchers to stop and waits for them to complete.
    /// In-flight job-processing tasks will continue to completion naturally.
    ///
    /// This method is idempotent and can be safely called multiple times.
    /// It blocks until all dispatchers have shut down.
    ///
    /// # Note
    ///
    /// You should call this method before dropping the WorkerPool to ensure
    /// clean shutdown. Dropping without calling stop() may leave orphaned tasks.
    pub async fn stop(&self) -> Result<(), Error> {
        // Check if already stopped (idempotent)
        if !self.is_running().await {
            tracing::debug!("Worker pool is already stopped");
            return Ok(());
        }

        tracing::info!("Shutting down worker pool...");

        // Close semaphore to prevent new jobs from starting
        self.inner.semaphore.close();

        // Signal all dispatchers to stop
        self.inner.shutdown.notify_waiters();

        // Stop the job queue cleanup task
        self.inner.queue.stop_cleanup().await;

        // Wait for all dispatchers to finish (with timeout)
        let mut handles = self.inner.dispatcher_handles.write().await;
        let dispatcher_count = handles.len();

        for (i, handle) in handles.drain(..).enumerate() {
            let timeout_result =
                tokio::time::timeout(self.inner.config.shutdown_timeout(), handle).await;

            match timeout_result {
                Ok(Ok(())) => {
                    // Dispatcher stopped cleanly
                    tracing::debug!("Dispatcher {} stopped cleanly", i);
                }
                Ok(Err(e)) => {
                    tracing::error!("Dispatcher {} panicked: {:?}", i, e);
                }
                Err(_) => {
                    tracing::warn!("Dispatcher {} did not stop within timeout", i);
                }
            }
        }

        tracing::info!(
            "Worker pool shut down ({} dispatchers stopped, in-flight tasks will complete)",
            dispatcher_count
        );

        Ok(())
    }

    /// Check if the worker pool is running
    pub async fn is_running(&self) -> bool {
        let handles = self.inner.dispatcher_handles.read().await;
        !handles.is_empty()
    }

    /// Get the number of active dispatchers
    pub async fn dispatcher_count(&self) -> usize {
        let handles = self.inner.dispatcher_handles.read().await;
        handles.len()
    }

    /// Get the number of available semaphore permits
    ///
    /// This indicates how many more jobs can be spawned before hitting the
    /// concurrency limit. A value of 0 means the system is at capacity.
    pub fn available_permits(&self) -> usize {
        self.inner.semaphore.available_permits()
    }

    /// Get the maximum number of concurrent jobs configured
    pub fn max_concurrent_jobs(&self) -> usize {
        self.inner.config.max_concurrent_jobs
    }

    /// Get the current number of jobs being processed
    ///
    /// This is calculated as: max_concurrent_jobs - available_permits
    pub fn active_job_count(&self) -> usize {
        self.inner.config.max_concurrent_jobs - self.inner.semaphore.available_permits()
    }
}
