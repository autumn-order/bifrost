use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use dioxus_logger::tracing;
use rand::Rng;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::server::model::worker::WorkerJob;

use super::{config::WorkerPoolConfig, context::DispatcherContext, handler::WorkerJobHandler};

/// Handle for a dispatcher task
pub(super) struct DispatcherHandle {
    pub id: usize,
    pub handle: JoinHandle<()>,
}

impl DispatcherHandle {
    /// Spawn a new dispatcher task
    ///
    /// Dispatchers continuously poll Redis for jobs and spawn tasks to process them.
    /// A semaphore limits the number of concurrent job-processing tasks, providing
    /// backpressure when the system is at capacity.
    pub fn spawn(id: usize, config: WorkerPoolConfig, context: DispatcherContext) -> Self {
        let handle = tokio::spawn(async move {
            tracing::info!("Dispatcher {} started", id);

            // Create dispatcher instance with owned state
            let mut dispatcher = Dispatcher::new(id, config, context);

            // Apply initial jittered delay to prevent thundering herd on startup
            dispatcher.apply_initial_jitter().await;

            // Run the main dispatcher loop
            dispatcher.run().await;

            tracing::info!("Dispatcher {} stopped", id);
        });

        Self { id, handle }
    }
}

/// Internal dispatcher instance that owns all state needed for job processing
struct Dispatcher {
    id: usize,
    config: WorkerPoolConfig,
    context: DispatcherContext,

    // Runtime state
    consecutive_errors: u32,
    jobs_dispatched: u64,
    job_buffer: VecDeque<WorkerJob>,
    prefetch_batch_size: usize,
    refill_threshold: usize,
}

impl Dispatcher {
    /// Create a new dispatcher instance
    fn new(id: usize, config: WorkerPoolConfig, context: DispatcherContext) -> Self {
        let prefetch_batch_size = config.prefetch_batch_size();
        let refill_threshold = (prefetch_batch_size / 4).max(1);

        Self {
            id,
            config,
            context,
            consecutive_errors: 0,
            jobs_dispatched: 0,
            job_buffer: VecDeque::with_capacity(prefetch_batch_size),
            prefetch_batch_size,
            refill_threshold,
        }
    }

    /// Apply initial jittered delay to prevent thundering herd on startup
    async fn apply_initial_jitter(&self) {
        let initial_jitter = self.config.dispatcher_initial_jitter();
        if initial_jitter.as_millis() > 0 {
            let jitter_ms = rand::rng().random_range(0..initial_jitter.as_millis() as u64);
            tracing::debug!(
                "Dispatcher {} applying initial jitter: {}ms before starting poll loop",
                self.id,
                jitter_ms
            );
            tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
        }
    }

    /// Main dispatcher loop
    ///
    /// Continuously polls Redis for jobs and spawns tasks to process them.
    /// Uses local buffering with batch popping to minimize Redis round-trips.
    ///
    /// # Concurrency Control
    /// - Semaphore limits concurrent job-processing tasks
    /// - When at capacity, `acquire_owned()` blocks until a task completes
    /// - Local buffer reduces Redis calls by 10-25x via batch popping
    /// - Buffer is refilled when running low (< 25% full or < 1 job)
    /// - Jittered sleep prevents all dispatchers from waking simultaneously
    async fn run(&mut self) {
        // Clone the shutdown signal Arc to avoid borrowing issues in the loop
        let shutdown_signal = Arc::clone(&self.context.shutdown_signal);

        loop {
            // Check for shutdown signal with biased select to prioritize shutdown
            tokio::select! {
                biased;

                _ = shutdown_signal.notified() => {
                    tracing::debug!(
                        "Dispatcher {} received shutdown signal ({} jobs left in buffer)",
                        self.id,
                        self.job_buffer.len()
                    );
                    break;
                }

                _ = self.process_iteration() => {
                    // Continue to next iteration
                }
            }
        }
    }

    /// Process one iteration of the dispatcher loop
    async fn process_iteration(&mut self) {
        // Refill buffer when running low
        self.refill_job_buffer().await;

        // Process a job from buffer if available
        if let Some(job) = self.job_buffer.pop_front() {
            // Try to acquire permit and dispatch job
            if !self.dispatch_job(job).await {
                // Semaphore closed, shutdown - this will be caught by the select
                return;
            }

            self.jobs_dispatched += 1;

            // Log stats periodically
            self.log_stats();
        } else if self.job_buffer.is_empty() {
            // Buffer is empty, sleep before next iteration
            self.sleep_with_backoff().await;
        }
    }

    /// Refill the job buffer when running low
    ///
    /// Uses batch popping to minimize Redis round-trips.
    async fn refill_job_buffer(&mut self) {
        if self.job_buffer.len() >= self.refill_threshold {
            return;
        }

        match self.context.queue.pop_batch(self.prefetch_batch_size).await {
            Ok(jobs) if !jobs.is_empty() => {
                self.consecutive_errors = 0;
                let fetched = jobs.len();
                self.job_buffer.extend(jobs);

                tracing::trace!(
                    "Dispatcher {} fetched {} jobs from Redis (buffer now has {} jobs)",
                    self.id,
                    fetched,
                    self.job_buffer.len()
                );
            }
            Ok(_) => {
                // Empty batch, queue is drained
                tracing::trace!("Dispatcher {} found empty queue", self.id);
            }
            Err(e) => {
                self.consecutive_errors += 1;
                tracing::error!(
                    "Dispatcher {} error fetching batch from queue (consecutive errors: {}): {:?}",
                    self.id,
                    self.consecutive_errors,
                    e
                );
            }
        }
    }

    /// Dispatch a single job for processing
    ///
    /// Acquires a semaphore permit and spawns a task to handle the job with timeout.
    /// Returns false if the semaphore is closed (shutdown), true otherwise.
    async fn dispatch_job(&self, job: WorkerJob) -> bool {
        // Acquire semaphore permit (blocks if at capacity)
        // This provides backpressure when system is overloaded
        let permit = match self.context.semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                // Semaphore closed, likely shutdown
                tracing::debug!("Dispatcher {} semaphore closed", self.id);
                return false;
            }
        };

        // Clone resources for the spawned task
        let db = Arc::clone(&self.context.db);
        let esi_client = Arc::clone(&self.context.esi_client);
        let timeout_duration = self.config.job_timeout();

        // Spawn task to process job with timeout
        // Tokio's work-stealing scheduler distributes tasks efficiently
        tokio::spawn(async move {
            Self::execute_job_with_timeout(job, db, esi_client, timeout_duration, permit).await;
        });

        true
    }

    /// Execute a job with timeout and handle the result
    ///
    /// Wraps job execution with timeout to prevent hung jobs.
    /// The semaphore permit is automatically released when this function completes.
    async fn execute_job_with_timeout(
        job: WorkerJob,
        db: Arc<DatabaseConnection>,
        esi_client: Arc<eve_esi::Client>,
        timeout_duration: Duration,
        _permit: tokio::sync::OwnedSemaphorePermit,
    ) {
        let handler = WorkerJobHandler::new(&db, &esi_client);

        // Wrap job execution with timeout to prevent hung jobs
        let result = tokio::time::timeout(timeout_duration, handler.handle(&job)).await;

        match result {
            Ok(Ok(())) => {
                // Job completed successfully
            }
            Ok(Err(e)) => {
                tracing::error!("Failed to process job {:?}: {:?}", job, e);
            }
            Err(_) => {
                tracing::error!(
                    "Job timed out after {} seconds: {:?}",
                    timeout_duration.as_secs(),
                    job
                );
            }
        }

        // Permit automatically dropped here, releasing semaphore slot
    }

    /// Log dispatcher statistics periodically
    fn log_stats(&self) {
        if self.jobs_dispatched % 100 == 0 {
            tracing::debug!(
                "Dispatcher {} has dispatched {} jobs ({} permits available, {} buffered)",
                self.id,
                self.jobs_dispatched,
                self.context.available_permits(),
                self.job_buffer.len()
            );
        }
    }

    /// Sleep with backoff when buffer is empty
    ///
    /// Uses error backoff if too many consecutive errors occurred,
    /// otherwise uses normal polling interval with jitter.
    async fn sleep_with_backoff(&mut self) {
        if self.consecutive_errors >= self.config.max_consecutive_errors {
            tracing::warn!(
                "Dispatcher {} backing off for {} seconds due to {} consecutive errors",
                self.id,
                self.config.error_backoff_seconds,
                self.consecutive_errors
            );
            tokio::time::sleep(Duration::from_secs(self.config.error_backoff_seconds)).await;
            self.consecutive_errors = 0;
        } else {
            // Queue empty or error, normal backoff with jitter
            let jitter = rand::rng().random_range(0..self.config.poll_interval_ms / 2);
            let delay = self.config.poll_interval_ms + jitter;
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    }
}
