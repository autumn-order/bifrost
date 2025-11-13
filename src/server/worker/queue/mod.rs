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
//! Jobs have a 24-hour TTL and are automatically cleaned up:
//! - Passive cleanup runs every 1000 job pushes (background task, non-blocking)
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

use lua::{CLEANUP_STALE_JOBS_SCRIPT, PUSH_JOB_SCRIPT};

use chrono::{DateTime, Utc};
use dioxus_logger::tracing;
use fred::prelude::*;

use crate::server::{error::Error, model::worker::WorkerJob};

const DEFAULT_QUEUE_NAME: &str = "bifrost:worker:queue";

/// Maximum age for jobs in the queue before they're considered stale (24 hours in milliseconds)
/// Jobs older than this will be removed by cleanup operations
pub(self) const JOB_TTL_MS: i64 = 24 * 60 * 60 * 1000;

/// Cleanup stale jobs every time this many jobs are pushed (1000 pushes)
/// This provides passive cleanup without requiring a separate background task
const CLEANUP_INTERVAL: u64 = 1000;

pub struct WorkerJobQueue {
    pool: Pool,
    /// Counter for tracking when to run cleanup (uses atomic operations)
    push_counter: std::sync::atomic::AtomicU64,
    /// Queue name in Redis (allows namespacing for test isolation)
    queue_name: String,
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
            push_counter: std::sync::atomic::AtomicU64::new(0),
            queue_name,
        }
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

        // Periodically cleanup stale jobs (every CLEANUP_INTERVAL pushes)
        if was_added {
            let count = self
                .push_counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count % CLEANUP_INTERVAL == 0 {
                // Run cleanup in background, don't wait for it
                let pool = self.pool.clone();
                let queue_name = self.queue_name.clone();
                tokio::spawn(async move {
                    if let Err(e) = Self::cleanup_stale_jobs_internal(&pool, &queue_name).await {
                        tracing::warn!("Failed to cleanup stale jobs: {}", e);
                    }
                });
            }
        }

        Ok(was_added)
    }

    /// Schedule job to be executed at provided time
    pub async fn schedule(&self, _job: WorkerJob, _time: DateTime<Utc>) -> Result<(), Error> {
        Ok(())
    }

    /// Retrieve earliest job from queue
    pub async fn pop(&self) -> Result<Option<WorkerJob>, Error> {
        Ok(None)
    }

    /// Retrieve all worker jobs of type without removing from queue
    pub async fn get_all_of_type(&self, _job: WorkerJob) -> Result<Vec<QueuedJob>, Error> {
        Ok(Vec::new())
    }

    /// Remove all jobs older than JOB_TTL_MS from the queue
    ///
    /// This method is called automatically during push operations, but can also be
    /// called manually for immediate cleanup.
    ///
    /// # Returns
    /// Returns the number of stale jobs that were removed from the queue.
    pub async fn cleanup_stale_jobs(&self) -> Result<u64, Error> {
        Self::cleanup_stale_jobs_internal(&self.pool, &self.queue_name).await
    }

    /// Internal implementation of cleanup that can be called from spawn
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
