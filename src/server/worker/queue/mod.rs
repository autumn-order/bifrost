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
//! ## Retry Metadata Storage
//!
//! Jobs support retry tracking with exponential backoff. To maintain proper deduplication while
//! tracking retry attempts, retry metadata is stored separately from the job identity:
//!
//! - **Job Identity**: Serialized job JSON used as ZSET member (for deduplication)
//! - **Retry Metadata**: Stored in separate Redis hash `{queue_name}:retry`
//!
//! This separation ensures that a fresh job and a retrying job are considered duplicates
//! (preventing the same job from being queued multiple times), while still preserving
//! retry attempt count and backoff information.
//!
//! ## TTL and Cleanup
//!
//! Jobs have a 1-hour TTL and are automatically cleaned up:
//! - Passive cleanup runs every 5 minutes (background task, non-blocking)
//! - Manual cleanup can be triggered via [`WorkerJobQueue::cleanup_stale_jobs`]
//! - Stale jobs (older than TTL) are removed to prevent queue bloat
//! - Orphaned retry metadata entries are also cleaned up during this process
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
    error::{worker::WorkerError, AppError},
    model::worker::{RetryMetadata, ScheduledWorkerJob, WorkerJob},
    worker::queue::config::WorkerQueueConfig,
};

/// Worker job queue with Redis backend.
///
/// Provides job enqueueing, scheduling, and deduplication using Redis as the backing store.
/// Jobs are stored with TTLs for automatic expiration and a background cleanup task removes
/// expired jobs periodically.
#[derive(Clone)]
pub struct WorkerQueue {
    inner: Arc<WorkerQueueRef>,
}

/// Internal worker queue reference with Redis pool and configuration.
///
/// Contains the Redis connection pool, queue configuration, and handles for background
/// cleanup tasks. This struct is wrapped in an Arc by `WorkerQueue` for cheap cloning.
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
                config,
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
    /// This is a convenience wrapper around `schedule` that uses the current time.
    ///
    /// # Arguments
    /// - `job` - Worker job to add to the queue
    ///
    /// # Returns
    /// - `Ok(true)` - Job was added to the queue
    /// - `Ok(false)` - Duplicate already exists in the queue
    /// - `Err(AppError::Worker)` - Serialization failed
    /// - `Err(AppError)` - Redis communication failed
    pub async fn push(&self, job: WorkerJob) -> Result<bool, AppError> {
        self.schedule(job, Utc::now(), None).await
    }

    /// Schedules a job to be executed at a specific time.
    ///
    /// Uses a Lua script to atomically check for duplicates and add the job to the queue
    /// with the specified timestamp. Jobs with identical serialized JSON are deduplicated.
    /// Retry metadata is stored separately in a Redis hash to avoid affecting deduplication.
    ///
    /// # Arguments
    /// - `job` - Worker job to add to the queue
    /// - `scheduled_at` - UTC timestamp when the job should be executed
    /// - `retry_metadata` - Optional retry metadata (attempt count, first failure time)
    ///
    /// # Returns
    /// - `Ok(true)` - Job was added to the queue
    /// - `Ok(false)` - Duplicate already exists in the queue
    /// - `Err(AppError::Worker)` - Serialization failed
    /// - `Err(AppError)` - Redis communication failed
    pub async fn schedule(
        &self,
        job: WorkerJob,
        scheduled_at: DateTime<Utc>,
        retry_metadata: Option<RetryMetadata>,
    ) -> Result<bool, AppError> {
        let serialized = serde_json::to_string(&job)
            .map_err(|e| AppError::Worker(WorkerError::Serialization(e.to_string())))?;
        let score = scheduled_at.timestamp_millis() as f64;

        // Execute Lua script atomically
        // Uses ZSCORE for O(1) duplicate check, then ZADD with serialized JSON as member
        let result: i64 = self
            .inner
            .pool
            .eval(
                PUSH_JOB_SCRIPT,
                vec![&self.inner.config.queue_name],
                vec![serialized.clone(), score.to_string()],
            )
            .await?;

        // result is 1 if added, 0 if duplicate exists
        let was_added = result == 1;

        // If job was added and has retry metadata, store it separately
        if was_added {
            if let Some(metadata) = retry_metadata {
                let retry_hash_key = format!("{}:retry", self.inner.config.queue_name);
                let metadata_json = serde_json::to_string(&metadata)
                    .map_err(|e| AppError::Worker(WorkerError::Serialization(e.to_string())))?;

                let _: () = self
                    .inner
                    .pool
                    .hset(&retry_hash_key, (&serialized, metadata_json))
                    .await?;
            }
        }

        Ok(was_added)
    }

    /// Retrieves the earliest due job from the queue with its scheduled timestamp.
    ///
    /// Uses a Lua script to atomically retrieve and remove the job with the lowest score
    /// (earliest timestamp) that is due for execution (score <= current time). Returns both
    /// the job and the timestamp it was originally scheduled for, allowing the worker handler
    /// to distinguish between jobs scheduled before downtime versus during downtime.
    ///
    /// Also retrieves and removes any associated retry metadata from the separate hash.
    ///
    /// # Returns
    /// - `Ok(Some(ScheduledWorkerJob))` - Job was popped from the queue with scheduled timestamp
    /// - `Ok(None)` - Queue is empty or no jobs are due yet
    /// - `Err(AppError::Worker)` - Deserialization failed
    /// - `Err(AppError)` - Redis communication failed
    pub async fn pop(&self) -> Result<Option<ScheduledWorkerJob>, AppError> {
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
                let score_millis: i64 = values[1].clone().convert()?;

                // Deserialize JSON back into WorkerJob
                let job: WorkerJob = serde_json::from_str(&serialized)
                    .map_err(|e| AppError::Worker(WorkerError::Serialization(e.to_string())))?;

                // Convert score (milliseconds) to DateTime<Utc>
                let scheduled_at =
                    DateTime::from_timestamp_millis(score_millis).ok_or_else(|| {
                        AppError::Worker(WorkerError::Serialization(format!(
                            "Invalid timestamp from Redis: {}",
                            score_millis
                        )))
                    })?;

                // Retrieve and remove retry metadata if it exists
                let retry_hash_key = format!("{}:retry", self.inner.config.queue_name);
                let retry_metadata: Option<String> =
                    self.inner.pool.hget(&retry_hash_key, &serialized).await?;

                let metadata = if let Some(metadata_json) = retry_metadata {
                    // Remove from hash now that we've retrieved it
                    let _: () = self.inner.pool.hdel(&retry_hash_key, &serialized).await?;

                    // Deserialize metadata
                    let parsed: RetryMetadata = serde_json::from_str(&metadata_json)
                        .map_err(|e| AppError::Worker(WorkerError::Serialization(e.to_string())))?;
                    Some(parsed)
                } else {
                    None
                };

                Ok(Some(match metadata {
                    Some(m) => ScheduledWorkerJob::with_retry(job, scheduled_at, m),
                    None => ScheduledWorkerJob::new(job, scheduled_at),
                }))
            }
        }
    }

    /// Gets the number of jobs currently in the queue.
    ///
    /// This method is useful for monitoring queue depth and ensuring
    /// jobs are being processed in a timely manner.
    ///
    /// # Returns
    /// - `Ok(usize)` - Number of jobs in the queue
    /// - `Err(AppError)` - Redis communication failed
    pub async fn len(&self) -> Result<usize, AppError> {
        let count: i64 = self.inner.pool.zcard(&self.inner.config.queue_name).await?;
        Ok(count as usize)
    }

    /// Checks if the queue is empty.
    ///
    /// # Returns
    /// - `Ok(true)` - Queue is empty
    /// - `Ok(false)` - Queue has jobs
    /// - `Err(AppError)` - Redis communication failed
    pub async fn is_empty(&self) -> Result<bool, AppError> {
        Ok(self.len().await? == 0)
    }

    /// Removes all jobs older than the configured TTL from the queue.
    ///
    /// This method is called automatically by the background cleanup task at regular
    /// intervals, but can also be called manually for immediate cleanup. Also cleans
    /// up orphaned retry metadata from the hash.
    ///
    /// # Returns
    /// - `Ok(u64)` - Number of stale jobs removed from the queue
    /// - `Err(AppError)` - Redis communication failed
    pub async fn cleanup_stale_jobs(&self) -> Result<u64, AppError> {
        Self::cleanup_stale_jobs_internal(&self.inner.config, &self.inner.pool).await
    }

    /// Internal implementation of cleanup that can be called from the background task.
    ///
    /// Performs the actual cleanup logic using Redis Lua script to remove stale jobs.
    /// Also removes orphaned retry metadata for jobs that no longer exist in the queue.
    /// This is separated from the public method to allow both manual and automatic cleanup.
    ///
    /// # Arguments
    /// - `config` - Queue configuration with TTL settings
    /// - `pool` - Redis connection pool
    ///
    /// # Returns
    /// - `Ok(u64)` - Number of stale jobs removed
    /// - `Err(AppError)` - Redis operation failed
    async fn cleanup_stale_jobs_internal(
        config: &WorkerQueueConfig,
        pool: &Pool,
    ) -> Result<u64, AppError> {
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

        // Clean up orphaned retry metadata
        // Get all retry metadata keys
        let retry_hash_key = format!("{}:retry", config.queue_name);
        let all_retry_keys: Vec<String> = pool.hkeys(&retry_hash_key).await?;

        if !all_retry_keys.is_empty() {
            // Check which jobs still exist in the queue
            let mut orphaned_keys = Vec::new();
            for key in all_retry_keys {
                let exists: Option<f64> = pool.zscore(&config.queue_name, &key).await?;
                if exists.is_none() {
                    orphaned_keys.push(key);
                }
            }

            // Remove orphaned metadata
            if !orphaned_keys.is_empty() {
                let orphaned_count = orphaned_keys.len();
                let _: () = pool.hdel(&retry_hash_key, orphaned_keys).await?;
                tracing::info!(
                    "Cleaned up {} orphaned retry metadata entries from worker queue",
                    orphaned_count
                );
            }
        }

        Ok(removed as u64)
    }
}
