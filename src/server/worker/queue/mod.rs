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

use lua::{CLEANUP_STALE_JOBS_SCRIPT, GET_ALL_OF_TYPE_SCRIPT, POP_JOB_SCRIPT, PUSH_JOB_SCRIPT};

use chrono::{DateTime, Utc};
use dioxus_logger::tracing;
use fred::prelude::*;

use crate::server::{
    error::{worker::WorkerError, Error},
    model::worker::WorkerJob,
};

const DEFAULT_QUEUE_NAME: &str = "bifrost:worker:queue";

/// Maximum age for jobs in the queue before they're considered stale (24 hours in milliseconds)
/// Jobs older than this will be removed by cleanup operations
const JOB_TTL_MS: i64 = 24 * 60 * 60 * 1000;

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

        // Periodically cleanup stale jobs (every 1000 job pushes)
        self.trigger_periodic_cleanup(was_added);

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

        // Periodically cleanup stale jobs (every 1000 job pushes)
        self.trigger_periodic_cleanup(was_added);

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

    /// Retrieve all worker jobs of type without removing from queue
    ///
    /// This method is useful for preventing duplicate job scheduling by checking what jobs
    /// are already in the queue. For example, when scheduling UpdateAllianceInfo jobs, you can
    /// retrieve all existing alliance jobs and exclude those IDs from your database query.
    ///
    /// # Returns
    ///
    /// Returns a vector of `QueuedJob` containing all jobs of the specified type currently in the queue,
    /// along with their scheduled execution times.
    ///
    /// # Note
    ///
    /// For affiliation batch jobs, this will return jobs but the character_ids field will be empty
    /// since the identity string only contains count and hash. The actual character IDs must be
    /// retrieved from the database when processing these jobs.
    pub async fn get_all_of_type(&self, job: WorkerJob) -> Result<Vec<QueuedJob>, Error> {
        // Determine the identity prefix based on the job type
        let prefix = match job {
            WorkerJob::UpdateCharacterInfo { .. } => "character:info:",
            WorkerJob::UpdateAllianceInfo { .. } => "alliance:info:",
            WorkerJob::UpdateCorporationInfo { .. } => "corporation:info:",
            WorkerJob::UpdateAffiliations { .. } => "affiliation:batch:",
        };

        // Execute Lua script to get all matching jobs
        let result: Vec<Value> = self
            .pool
            .eval(GET_ALL_OF_TYPE_SCRIPT, vec![&self.queue_name], vec![prefix])
            .await?;

        // Parse results into QueuedJob structs
        let mut jobs = Vec::new();

        // Results come in pairs: [identity1, score1, identity2, score2, ...]
        for chunk in result.chunks(2) {
            if chunk.len() == 2 {
                let identity: String = chunk[0].clone().convert()?;
                let score: f64 = chunk[1].clone().convert()?;

                // Convert score (milliseconds) to DateTime
                let timestamp_ms = score as i64;
                let scheduled_at =
                    DateTime::from_timestamp_millis(timestamp_ms).ok_or_else(|| {
                        Error::from(WorkerError::InvalidJobIdentity(format!(
                            "Invalid timestamp: {}",
                            timestamp_ms
                        )))
                    })?;

                // Parse identity back into WorkerJob
                // For affiliation batches, this will fail to parse due to missing character IDs
                // We handle this specially by creating an empty affiliation job
                let job = match WorkerJob::parse_identity(&identity) {
                    Ok(job) => job,
                    Err(_) if identity.starts_with("affiliation:batch:") => {
                        // For affiliation batches, create a job with empty character_ids
                        // since the actual IDs must be retrieved from the database
                        WorkerJob::UpdateAffiliations {
                            character_ids: Vec::new(),
                        }
                    }
                    Err(e) => return Err(e),
                };

                jobs.push(QueuedJob { job, scheduled_at });
            }
        }

        Ok(jobs)
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

    /// Trigger periodic cleanup if a job was added and cleanup interval is reached
    ///
    /// This method is called by push and schedule to periodically clean up stale jobs.
    /// Cleanup runs in the background without blocking the caller.
    fn trigger_periodic_cleanup(&self, was_added: bool) {
        if was_added {
            let count = self
                .push_counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // Check if we've reached the cleanup interval
            // count + 1 because fetch_add returns the value BEFORE incrementing
            if (count + 1) % CLEANUP_INTERVAL == 0 {
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
