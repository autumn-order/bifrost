//! Worker queue for Bifrost's interval window scheduler
//!
//! All jobs are scheduled in intervals of up to a 30 minute window where the jobs
//! are staggered evenly to prevent bursts of jobs. This queue provides methods
//! to push and schedule jobs
//!
//! ## Duplicate Guardrails
//! The following methods can be used as guardrails to prevent the duplicate scheduling of jobs:
//!
//! 1. [`WorkerJobQueue::push`] & [`WorkerJobQueue::schedule`] Prevents the insertion of duplicate jobs
//!    already in queue
//! 2. [`WorkerJobQueue::get_all_of_type`]: retrieve all worker jobs of a type, you can then extract the IDs
//!    to prevent retrieving duplicate IDs from the database.
//!
//! ## TTL and Cleanup
//!
//! Jobs have a 1-hour TTL and are automatically cleaned up:
//! - Passive cleanup runs every 5 minutes (background task, non-blocking)
//! - Manual cleanup can be triggered via [`WorkerJobQueue::cleanup_stale_jobs`]
//! - Stale jobs (older than TTL) are removed to prevent queue bloat
//!
//! ## How this will be implemented
//!
//! 1. Call `get_all_of_type` when scheduling for example [`WorkerJob::UpdateAllianceInfo`].
//! 2. If our max batch is 200, we still have 50 in queue because the queue ran past the 30 minute
//!    window, subtract 50 from batch, then retrieve from database where alliance IDs are not in the list
//!    of already scheduled alliance IDs
//! 3. Schedule all jobs staggered across across a 30 minute window, offset by the last overflowed
//!    job's scheduled time
//!
//! Note: Current tracking keys system will be removed as that was simply a workaround to prevent duplicates
//! with apalis.
pub mod config;

mod lua;

use lua::{CLEANUP_STALE_JOBS_SCRIPT, POP_JOB_SCRIPT, PUSH_JOB_SCRIPT};

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use chrono::{DateTime, Utc};
use dioxus_logger::tracing;
use fred::prelude::*;

use crate::server::{
    error::{worker::WorkerError, Error},
    model::worker::WorkerJob,
    worker::queue::config::WorkerQueueConfig,
};

#[derive(Clone)]
pub struct WorkerQueue {
    inner: Arc<WorkerQueueRef>,
}

#[derive(Clone)]
pub struct WorkerQueueRef {
    pool: Pool,
    config: WorkerQueueConfig,
    /// Handle to the background cleanup task
    cleanup_task_handle: std::sync::Arc<tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Shutdown flag for the cleanup task
    shutdown_flag: std::sync::Arc<AtomicBool>,
}

impl WorkerQueue {
    /// Creates a new worker queue with default configuration.
    ///
    /// Initializes a queue with default queue name, 1-hour job TTL, and 5-minute cleanup interval.
    ///
    /// # Arguments
    /// - `pool` - Redis connection pool for queue storage
    ///
    /// # Returns
    /// - `WorkerQueue` - New queue instance with default configuration
    pub fn new(pool: Pool) -> Self {
        Self::with_config(pool, WorkerQueueConfig::default())
    }

    /// Creates a new worker queue with custom configuration.
    ///
    /// Initializes a queue with custom settings for queue name, job TTL, and cleanup interval.
    /// Useful for testing with custom queue names or different TTL values.
    ///
    /// # Arguments
    /// - `pool` - Redis connection pool for queue storage
    /// - `config` - Custom queue configuration
    ///
    /// # Returns
    /// - `WorkerQueue` - New queue instance with custom configuration
    pub fn with_config(pool: Pool, config: WorkerQueueConfig) -> Self {
        Self {
            inner: Arc::new(WorkerQueueRef {
                pool,
                config: config,
                cleanup_task_handle: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
                shutdown_flag: std::sync::Arc::new(AtomicBool::new(false)),
            }),
        }
    }

    /// Starts the background cleanup task for periodic removal of stale jobs.
    ///
    /// Spawns a tokio task that runs every cleanup_interval to remove jobs older than
    /// the configured job_ttl. The task respects the shutdown flag and exits cleanly.
    /// This method is idempotent - calling it when already running has no effect.
    ///
    /// # Note
    /// The task will stop when `stop_cleanup()` is called or the queue is dropped.
    pub async fn start_cleanup(&self) {
        let mut handle = self.inner.cleanup_task_handle.write().await;

        if handle.is_some() {
            tracing::debug!("Worker queue cleanup task is already running");
            return;
        }

        let config = self.inner.config.clone();
        let pool = self.inner.pool.clone();
        let shutdown_flag = self.inner.shutdown_flag.clone();

        let task_handle = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(config.cleanup_interval);

            tracing::info!(
                "Worker queue cleanup task started with interval of {:?} seconds",
                config.cleanup_interval.as_secs()
            );

            loop {
                // Check shutdown flag before waiting
                if shutdown_flag.load(Ordering::Relaxed) {
                    tracing::info!("Worker queue cleanup task received shutdown signal");
                    break;
                }

                tokio::select! {
                    biased;

                    _ = interval_timer.tick() => {
                        // Check again after waking up
                        if shutdown_flag.load(Ordering::Relaxed) {
                            tracing::info!("Worker queue cleanup task received shutdown signal");
                            break;
                        }

                        if let Err(e) = Self::cleanup_stale_jobs_internal(&config, &pool).await {
                            tracing::warn!("Failed to cleanup stale worker queue jobs: {}", e);
                        }
                    }
                }
            }

            tracing::info!("Worker queue cleanup task stopped");
        });

        *handle = Some(task_handle);
    }

    /// Stops the background cleanup task gracefully.
    ///
    /// Signals the cleanup task to stop and waits for it to complete. Resets the shutdown
    /// flag for potential restart. Safe to call even if the cleanup task is not running.
    pub async fn stop_cleanup(&self) {
        // Signal shutdown
        self.inner.shutdown_flag.store(true, Ordering::Relaxed);

        // Wait for the task to finish
        let mut handle = self.inner.cleanup_task_handle.write().await;
        if let Some(task_handle) = handle.take() {
            tracing::debug!("Waiting for worker queue cleanup task to stop");
            match task_handle.await {
                Ok(()) => {
                    tracing::debug!("Worker queue cleanup task stopped cleanly");
                }
                Err(e) if e.is_panic() => {
                    tracing::error!("Worker queue cleanup task panicked: {:?}", e);
                }
                Err(e) if e.is_cancelled() => {
                    tracing::warn!("Worker queue cleanup task was cancelled");
                }
                Err(e) => {
                    tracing::error!("Worker queue cleanup task failed: {:?}", e);
                }
            }
        }

        // Reset the shutdown flag for potential restart
        self.inner.shutdown_flag.store(false, Ordering::Relaxed);
    }

    /// Checks if the cleanup task is currently running.
    ///
    /// # Returns
    /// - `true` - Cleanup task is active
    /// - `false` - Cleanup task is stopped
    pub async fn is_cleanup_running(&self) -> bool {
        let handle = self.inner.cleanup_task_handle.read().await;
        handle.is_some()
    }

    /// Pushes a job to be executed as soon as possible.
    ///
    /// Uses a Lua script to atomically check for duplicates and add the job to the queue
    /// with current timestamp. Jobs with identical serialized JSON are deduplicated.
    ///
    /// # Arguments
    /// - `job` - Worker job to add to the queue
    ///
    /// # Returns
    /// - `Ok(true)` - Job was added to the queue
    /// - `Ok(false)` - Duplicate already exists in the queue
    /// - `Err(Error::WorkerError)` - Serialization failed
    /// - `Err(Error)` - Redis communication failed
    pub async fn push(&self, job: WorkerJob) -> Result<bool, Error> {
        let serialized = serde_json::to_string(&job)
            .map_err(|e| Error::WorkerError(WorkerError::SerializationError(e.to_string())))?;
        let score = Utc::now().timestamp_millis() as f64;

        // Execute Lua script atomically
        // Uses ZSCORE for O(1) duplicate check, then ZADD with serialized JSON as member
        let result: i64 = self
            .inner
            .pool
            .eval(
                PUSH_JOB_SCRIPT,
                vec![&self.inner.config.queue_name],
                vec![serialized, score.to_string()],
            )
            .await?;

        // result is 1 if added, 0 if duplicate exists
        let was_added = result == 1;

        Ok(was_added)
    }

    /// Schedules a job to be executed at a specific time.
    ///
    /// Uses a Lua script to atomically check for duplicates and add the job to the queue
    /// with the specified timestamp. Jobs with identical serialized JSON are deduplicated.
    ///
    /// # Arguments
    /// - `job` - Worker job to add to the queue
    /// - `time` - UTC timestamp when the job should be executed
    ///
    /// # Returns
    /// - `Ok(true)` - Job was added to the queue
    /// - `Ok(false)` - Duplicate already exists in the queue
    /// - `Err(Error::WorkerError)` - Serialization failed
    /// - `Err(Error)` - Redis communication failed
    pub async fn schedule(&self, job: WorkerJob, time: DateTime<Utc>) -> Result<bool, Error> {
        let serialized = serde_json::to_string(&job)
            .map_err(|e| Error::WorkerError(WorkerError::SerializationError(e.to_string())))?;
        let score = time.timestamp_millis() as f64;

        // Execute Lua script atomically
        // Uses ZSCORE for O(1) duplicate check, then ZADD with serialized JSON as member
        let result: i64 = self
            .inner
            .pool
            .eval(
                PUSH_JOB_SCRIPT,
                vec![&self.inner.config.queue_name],
                vec![serialized, score.to_string()],
            )
            .await?;

        // result is 1 if added, 0 if duplicate exists
        let was_added = result == 1;

        Ok(was_added)
    }

    /// Retrieves the earliest due job from the queue.
    ///
    /// Uses a Lua script to atomically retrieve and remove the job with the lowest score
    /// (earliest timestamp) that is due for execution (score <= current time).
    ///
    /// # Returns
    /// - `Ok(Some(WorkerJob))` - Job was popped from the queue
    /// - `Ok(None)` - Queue is empty or no jobs are due yet
    /// - `Err(Error::WorkerError)` - Deserialization failed
    /// - `Err(Error)` - Redis communication failed
    pub async fn pop(&self) -> Result<Option<WorkerJob>, Error> {
        // Execute Lua script to atomically pop earliest job that is due
        let now = Utc::now().timestamp_millis();
        let result: Option<Vec<Value>> = self
            .inner
            .pool
            .eval(
                POP_JOB_SCRIPT,
                vec![&self.inner.config.queue_name],
                vec![now.to_string()],
            )
            .await?;

        match result {
            None => Ok(None),
            Some(values) => {
                // Extract serialized JSON from result
                // values[0] is the serialized job, values[1] is the score
                if values.is_empty() {
                    return Ok(None);
                }

                let serialized: String = values[0].clone().convert()?;

                // Deserialize JSON back into WorkerJob
                let job: WorkerJob = serde_json::from_str(&serialized).map_err(|e| {
                    Error::WorkerError(WorkerError::SerializationError(e.to_string()))
                })?;

                Ok(Some(job))
            }
        }
    }

    /// Removes all jobs older than the configured TTL from the queue.
    ///
    /// This method is called automatically by the background cleanup task at regular
    /// intervals, but can also be called manually for immediate cleanup.
    ///
    /// # Returns
    /// - `Ok(u64)` - Number of stale jobs removed from the queue
    /// - `Err(Error)` - Redis communication failed
    pub async fn cleanup_stale_jobs(&self) -> Result<u64, Error> {
        Self::cleanup_stale_jobs_internal(&self.inner.config, &self.inner.pool).await
    }

    /// Internal implementation of cleanup that can be called from the background task.
    ///
    /// Performs the actual cleanup logic using Redis Lua script to remove stale jobs.
    /// This is separated from the public method to allow both manual and automatic cleanup.
    ///
    /// # Arguments
    /// - `config` - Queue configuration with TTL settings
    /// - `pool` - Redis connection pool
    ///
    /// # Returns
    /// - `Ok(u64)` - Number of stale jobs removed
    /// - `Err(Error)` - Redis operation failed
    async fn cleanup_stale_jobs_internal(
        config: &WorkerQueueConfig,
        pool: &Pool,
    ) -> Result<u64, Error> {
        let cutoff_timestamp = Utc::now().timestamp_millis() - config.job_ttl.as_millis() as i64;
        let cutoff_score = cutoff_timestamp as f64;

        let removed: i64 = pool
            .eval(
                CLEANUP_STALE_JOBS_SCRIPT,
                vec![&config.queue_name],
                vec![cutoff_score.to_string()],
            )
            .await?;

        if removed > 0 {
            tracing::info!("Cleaned up {} stale jobs from worker queue", removed);
        }

        Ok(removed as u64)
    }
}
