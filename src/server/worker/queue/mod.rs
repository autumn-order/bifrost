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
mod lua;

#[cfg(test)]
mod tests;

use lua::{CLEANUP_STALE_JOBS_SCRIPT, POP_JOB_SCRIPT, PUSH_JOB_SCRIPT};

use chrono::{DateTime, Utc};
use dioxus_logger::tracing;
use fred::prelude::*;

use crate::server::{error::Error, model::worker::WorkerJob};

const DEFAULT_QUEUE_NAME: &str = "bifrost:worker:queue";

/// Maximum age for jobs in the queue before they're considered stale (1 hour in milliseconds)
/// Jobs older than this will be removed by cleanup operations
const JOB_TTL_MS: i64 = 60 * 60 * 1000;

/// Cleanup interval in milliseconds (5 minutes)
/// Cleanup will run at most once per this interval
const CLEANUP_INTERVAL_MS: i64 = 5 * 60 * 1000;

pub struct WorkerJobQueue {
    pool: Pool,
    /// Queue name in Redis (allows namespacing for test isolation)
    queue_name: String,
    /// Handle to the background cleanup task
    cleanup_task_handle: std::sync::Arc<tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Shutdown signal for the cleanup task
    shutdown_signal: std::sync::Arc<tokio::sync::Notify>,
}

pub struct QueuedJob {
    pub job: WorkerJob,
    pub scheduled_at: DateTime<Utc>,
}

impl WorkerJobQueue {
    pub fn new(pool: Pool) -> Self {
        Self::with_queue_name(pool, DEFAULT_QUEUE_NAME.to_string())
    }

    /// Create a new WorkerJobQueue with a custom queue name (useful for testing)
    pub fn with_queue_name(pool: Pool, queue_name: String) -> Self {
        Self {
            pool,
            queue_name,
            cleanup_task_handle: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
            shutdown_signal: std::sync::Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Start the background cleanup task for periodic removal of stale jobs
    ///
    /// This task runs every CLEANUP_INTERVAL_MS and removes jobs older than JOB_TTL_MS.
    /// The task will stop when [`Self::stop_cleanup`] is called or the queue is dropped.
    ///
    /// This method is idempotent - calling it multiple times has no effect if cleanup
    /// is already running.
    pub async fn start_cleanup(&self) {
        let mut handle = self.cleanup_task_handle.write().await;

        if handle.is_some() {
            tracing::debug!("Queue cleanup task is already running");
            return;
        }

        let pool = self.pool.clone();
        let queue_name = self.queue_name.clone();
        let shutdown_signal = self.shutdown_signal.clone();

        let task_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(
                CLEANUP_INTERVAL_MS as u64,
            ));

            tracing::info!("Queue cleanup task started");

            loop {
                tokio::select! {
                    biased;

                    _ = shutdown_signal.notified() => {
                        tracing::info!("Queue cleanup task received shutdown signal");
                        break;
                    }

                    _ = interval.tick() => {
                        if let Err(e) = Self::cleanup_stale_jobs_internal(&pool, &queue_name).await {
                            tracing::warn!("Failed to cleanup stale jobs: {}", e);
                        }
                    }
                }
            }

            tracing::info!("Queue cleanup task stopped");
        });

        *handle = Some(task_handle);
    }

    /// Stop the background cleanup task gracefully
    ///
    /// This method signals the cleanup task to stop and waits for it to complete.
    /// It is safe to call even if the cleanup task is not running.
    pub async fn stop_cleanup(&self) {
        // Signal shutdown
        self.shutdown_signal.notify_waiters();

        // Wait for the task to finish
        let mut handle = self.cleanup_task_handle.write().await;
        if let Some(task_handle) = handle.take() {
            tracing::debug!("Waiting for queue cleanup task to stop");
            match task_handle.await {
                Ok(()) => {
                    tracing::debug!("Queue cleanup task stopped cleanly");
                }
                Err(e) if e.is_panic() => {
                    tracing::error!("Queue cleanup task panicked: {:?}", e);
                }
                Err(e) if e.is_cancelled() => {
                    tracing::warn!("Queue cleanup task was cancelled");
                }
                Err(e) => {
                    tracing::error!("Queue cleanup task failed: {:?}", e);
                }
            }
        }
    }

    /// Check if the cleanup task is currently running
    pub async fn is_cleanup_running(&self) -> bool {
        let handle = self.cleanup_task_handle.read().await;
        handle.is_some()
    }

    /// Push a job to be executed as soon as possible
    ///
    /// Uses a Lua script to atomically check for duplicates and add the job to the queue.
    /// Jobs with the same identity will not be added if they already exist in the queue.
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` if the job was added to the queue.
    /// Returns `Ok(false)` if a duplicate already exists in the queue.
    /// Returns `Err` on Redis communication errors, serialization errors, or validation errors.
    pub async fn push(&self, job: WorkerJob) -> Result<bool, Error> {
        let identity = job.identity()?;
        let score = Utc::now().timestamp_millis() as f64;

        // Execute Lua script atomically
        // Uses ZSCORE for O(1) duplicate check, then ZADD with identity as member
        // Note: For affiliation batches, identity only contains count+hash, not actual character IDs
        let result: i64 = self
            .pool
            .eval(
                PUSH_JOB_SCRIPT,
                vec![&self.queue_name],
                vec![identity, score.to_string()],
            )
            .await?;

        // result is 1 if added, 0 if duplicate exists
        let was_added = result == 1;

        Ok(was_added)
    }

    /// Schedule job to be executed at provided time
    ///
    /// Uses a Lua script to atomically check for duplicates and add the job to the queue.
    /// Jobs with the same identity will not be added if they already exist in the queue.
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` if the job was added to the queue.
    /// Returns `Ok(false)` if a duplicate already exists in the queue.
    /// Returns `Err` on Redis communication errors, serialization errors, or validation errors.
    pub async fn schedule(&self, job: WorkerJob, time: DateTime<Utc>) -> Result<bool, Error> {
        let identity = job.identity()?;
        let score = time.timestamp_millis() as f64;

        // Execute Lua script atomically
        // Uses ZSCORE for O(1) duplicate check, then ZADD with identity as member
        // Note: For affiliation batches, identity only contains count+hash, not actual character IDs
        let result: i64 = self
            .pool
            .eval(
                PUSH_JOB_SCRIPT,
                vec![&self.queue_name],
                vec![identity, score.to_string()],
            )
            .await?;

        // result is 1 if added, 0 if duplicate exists
        let was_added = result == 1;

        Ok(was_added)
    }

    /// Retrieve earliest job from queue
    ///
    /// Uses a Lua script to atomically retrieve and remove the job with the lowest score
    /// (earliest timestamp) from the sorted set.
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(WorkerJob))` if a job was popped from the queue.
    /// Returns `Ok(None)` if the queue is empty.
    /// Returns `Err` on Redis communication errors or deserialization errors.
    ///
    /// # Note
    ///
    /// For affiliation batch jobs, this will fail because it has not yet been implemented
    /// for this new job scheduler system.
    pub async fn pop(&self) -> Result<Option<WorkerJob>, Error> {
        // Execute Lua script to atomically pop earliest job that is due
        let now = Utc::now().timestamp_millis();
        let result: Option<Vec<Value>> = self
            .pool
            .eval(
                POP_JOB_SCRIPT,
                vec![&self.queue_name],
                vec![now.to_string()],
            )
            .await?;

        match result {
            None => Ok(None),
            Some(values) => {
                // Extract identity string from result
                // values[0] is the identity, values[1] is the score
                if values.is_empty() {
                    return Ok(None);
                }

                let identity: String = values[0].clone().convert()?;

                // Parse identity back into WorkerJob
                let job = WorkerJob::parse_identity(&identity)?;

                Ok(Some(job))
            }
        }
    }

    /// Remove all jobs older than JOB_TTL_MS from the queue
    ///
    /// This method is called automatically by the background cleanup task every
    /// CLEANUP_INTERVAL_MS, but can also be called manually for immediate cleanup.
    ///
    /// # Returns
    /// Returns the number of stale jobs that were removed from the queue.
    pub async fn cleanup_stale_jobs(&self) -> Result<u64, Error> {
        Self::cleanup_stale_jobs_internal(&self.pool, &self.queue_name).await
    }

    /// Internal implementation of cleanup that can be called from the background task
    async fn cleanup_stale_jobs_internal(pool: &Pool, queue_name: &str) -> Result<u64, Error> {
        let cutoff_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS;
        let cutoff_score = cutoff_timestamp as f64;

        let removed: i64 = pool
            .eval(
                CLEANUP_STALE_JOBS_SCRIPT,
                vec![queue_name],
                vec![cutoff_score.to_string()],
            )
            .await?;

        if removed > 0 {
            tracing::info!("Cleaned up {} stale jobs from queue", removed);
        }

        Ok(removed as u64)
    }
}

impl Drop for WorkerJobQueue {
    fn drop(&mut self) {
        // Signal shutdown when queue is dropped
        self.shutdown_signal.notify_waiters();
    }
}
