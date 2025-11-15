use std::sync::Arc;
use std::time::Duration;

use dioxus_logger::tracing;
use rand::Rng;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::server::model::worker::WorkerJob;

use super::{
    config::WorkerPoolConfig,
    context::{DispatcherBuffer, DispatcherContext},
    handler::WorkerJobHandler,
};

/// Handle for a dispatcher task
pub(super) struct DispatcherHandle {
    pub(super) id: usize,
    pub(super) handle: JoinHandle<()>,
}

impl DispatcherHandle {
    /// Check if the dispatcher task has finished
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

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
///
/// The job buffer is stored externally in DispatcherContext to survive panics
/// even with panic = "abort" set. Each dispatcher has exclusive access to its
/// own buffer slot during normal operation (no contention).
struct Dispatcher {
    id: usize,
    config: WorkerPoolConfig,
    context: DispatcherContext,

    // Runtime state
    consecutive_errors: u32,
    jobs_dispatched: u64,
    buffer: Arc<DispatcherBuffer>,
    prefetch_batch_size: usize,
    refill_threshold: usize,
}

impl Dispatcher {
    /// Create a new dispatcher instance
    fn new(id: usize, config: WorkerPoolConfig, context: DispatcherContext) -> Self {
        let prefetch_batch_size = config.prefetch_batch_size();
        let refill_threshold = (prefetch_batch_size / 4).max(1);

        // Get reference to this dispatcher's persistent buffer
        let buffer = context
            .get_buffer(id)
            .expect("Dispatcher buffer must exist for valid dispatcher ID")
            .clone();

        Self {
            id,
            config,
            context,
            consecutive_errors: 0,
            jobs_dispatched: 0,
            buffer,
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
        let shutdown_signal: Arc<tokio::sync::Notify> = Arc::clone(&self.context.shutdown_signal);

        loop {
            // Check for shutdown signal with biased select to prioritize shutdown
            tokio::select! {
                biased;

                _ = shutdown_signal.notified() => {
                    let buffer_len = self.buffer.lock().await.len();
                    tracing::debug!(
                        "Dispatcher {} received shutdown signal ({} jobs left in buffer)",
                        self.id,
                        buffer_len
                    );
                    break;
                }

                _ = self.process_iteration() => {
                    // Continue to next iteration
                }
            }
        }

        // Return any buffered jobs back to the queue to prevent job loss
        self.shutdown_cleanup().await;
    }

    /// Process one iteration of the dispatcher loop
    async fn process_iteration(&mut self) {
        // Refill buffer when running low
        self.refill_job_buffer().await;

        // Process a job from buffer if available
        let job = {
            let mut buffer = self.buffer.lock().await;
            buffer.pop_front()
        };

        if let Some(job) = job {
            // Try to acquire permit and dispatch job
            match self.dispatch_job(job).await {
                Ok(()) => {
                    // Job dispatched successfully
                    self.jobs_dispatched += 1;
                    self.log_stats().await;
                }
                Err(job) => {
                    // Failed to dispatch (semaphore closed before acquire)
                    // Return job to buffer so it can be pushed back to queue during shutdown
                    let mut buffer = self.buffer.lock().await;
                    buffer.push_front(job);
                    return;
                }
            }
        } else {
            // Buffer is empty, sleep before next iteration
            self.sleep_with_backoff().await;
        }
    }

    /// Refill the job buffer when running low
    ///
    /// Uses batch popping to minimize Redis round-trips.
    async fn refill_job_buffer(&mut self) {
        let current_len = self.buffer.lock().await.len();
        if current_len >= self.refill_threshold {
            return;
        }

        match self.context.queue.pop_batch(self.prefetch_batch_size).await {
            Ok(jobs) if !jobs.is_empty() => {
                self.consecutive_errors = 0;
                let fetched = jobs.len();

                let mut buffer = self.buffer.lock().await;
                buffer.extend(jobs);
                let new_len = buffer.len();
                drop(buffer);

                tracing::trace!(
                    "Dispatcher {} fetched {} jobs from Redis (buffer now has {} jobs)",
                    self.id,
                    fetched,
                    new_len
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
    /// A wrapper task monitors for panics and logs them appropriately.
    ///
    /// # Returns
    /// - `Ok(())` if the job was successfully dispatched
    /// - `Err(job)` if shutting down or semaphore closed (returns the job)
    async fn dispatch_job(&self, job: WorkerJob) -> Result<(), WorkerJob> {
        // Check shutdown BEFORE acquiring semaphore to avoid race condition
        // This prevents interpreting semaphore closure as an error during shutdown
        if self.context.is_shutting_down() {
            tracing::debug!(
                "Dispatcher {} skipping job dispatch due to shutdown",
                self.id
            );
            return Err(job);
        }

        // Acquire semaphore permit (blocks if at capacity)
        // This provides backpressure when system is overloaded
        let permit = match self.context.semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                // Semaphore closed - should only happen during shutdown
                tracing::debug!(
                    "Dispatcher {} semaphore closed before acquiring permit",
                    self.id
                );
                return Err(job);
            }
        };

        // Clone resources for the spawned task
        let db = Arc::clone(&self.context.db);
        let esi_client = Arc::clone(&self.context.esi_client);
        let timeout_duration = self.config.job_timeout();

        // Capture job identity for panic logging
        let job_identity = job.identity().unwrap_or_else(|_| format!("{:?}", job));

        // Spawn wrapper task that monitors for panics
        // The wrapper awaits the inner job task and logs any panics that occur
        tokio::spawn(async move {
            let handle = tokio::spawn(async move {
                Self::execute_job_with_timeout(job, db, esi_client, timeout_duration, permit).await;
            });

            // Await the inner task and check for panic
            if let Err(e) = handle.await {
                if e.is_panic() {
                    tracing::error!("Job {:?} panicked: {:?}", job_identity, e);
                } else if e.is_cancelled() {
                    tracing::warn!("Job {:?} was cancelled", job_identity);
                }
            }
        });

        Ok(())
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
    async fn log_stats(&self) {
        if self.jobs_dispatched % 100 == 0 {
            tracing::debug!(
                "Dispatcher {} has dispatched {} jobs ({} permits available, {} buffered)",
                self.id,
                self.jobs_dispatched,
                self.context.available_permits(),
                self.buffer.lock().await.len()
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

    /// Cleanup buffered jobs on shutdown
    ///
    /// Returns all jobs remaining in the buffer back to the queue to prevent job loss.
    /// Jobs are pushed back with their original scheduled time (now) so they can be
    /// processed by other dispatchers or when the pool restarts.
    ///
    /// Uses batch push for efficient single-round-trip operation.
    /// Times out after 5 seconds to prevent hanging indefinitely if Redis is unavailable.
    async fn shutdown_cleanup(&mut self) {
        let buffer_size = self.buffer.lock().await.len();
        if buffer_size == 0 {
            return;
        }
        tracing::info!(
            "Dispatcher {} returning {} buffered jobs back to queue",
            self.id,
            buffer_size
        );

        // Collect all jobs with current timestamp for immediate execution
        let now = chrono::Utc::now();
        let jobs: Vec<_> = {
            let mut buffer = self.buffer.lock().await;
            buffer.drain(..).map(|job| (job, now)).collect()
        };

        // Give ourselves 5 seconds to return jobs to prevent hanging on Redis issues
        let cleanup_timeout = Duration::from_secs(5);
        let result =
            tokio::time::timeout(cleanup_timeout, self.context.queue.push_batch(jobs)).await;

        match result {
            Ok(Ok(added)) => {
                if added == buffer_size {
                    tracing::info!(
                        "Dispatcher {} successfully returned all {} jobs to queue",
                        self.id,
                        added
                    );
                } else {
                    tracing::warn!(
                        "Dispatcher {} returned {}/{} jobs to queue ({} were duplicates)",
                        self.id,
                        added,
                        buffer_size,
                        buffer_size - added
                    );
                }
            }
            Ok(Err(e)) => {
                tracing::error!(
                    "Dispatcher {} failed to return {} jobs to queue during shutdown: {:?}",
                    self.id,
                    buffer_size,
                    e
                );
            }
            Err(_) => {
                tracing::error!(
                    "Dispatcher {} timed out returning {} jobs to queue after {}s (jobs may be lost)",
                    self.id,
                    buffer_size,
                    cleanup_timeout.as_secs()
                );
            }
        }
    }
}
