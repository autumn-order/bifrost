//! Worker job handler for processing background tasks with retry logic.
//!
//! This module provides the `WorkerJobHandler` that executes different types of worker
//! jobs including EVE Online data updates. Each job type is dispatched to the appropriate
//! service method with comprehensive error handling, retry logic, and logging.
//!
//! # Retry Strategy
//!
//! The handler implements intelligent retry logic with exponential backoff and jitter to
//! prevent infinite retry loops and thundering herd problems:
//!
//! ## Exponential Backoff (Transient Errors)
//!
//! For transient errors (5xx responses, network issues, database connection failures):
//! - **Base delay**: 30 seconds
//! - **Multiplier**: 2^attempt_count
//! - **Jitter**: ±25% randomization to prevent synchronized retries
//! - **Cap**: 30 minutes maximum delay
//! - **Max attempts**: 10 retries (~3.5 hours total)
//!
//! Example progression:
//! - Attempt 1: ~30s (22.5s - 37.5s with jitter)
//! - Attempt 2: ~60s (45s - 75s)
//! - Attempt 3: ~120s (90s - 150s)
//! - Attempt 4: ~240s (3-5 min)
//! - Attempt 5: ~480s (6-10 min)
//! - Attempt 6: ~960s (12-20 min)
//! - Attempts 7-10: ~1800s (22.5-37.5 min, capped)
//!
//! ## Rate Limit Handling (429 Responses)
//!
//! For ESI rate limits, the handler uses the `retry_after` duration provided by ESI
//! plus a random stagger offset (0-15 minutes) to prevent thundering herd:
//! - **Base delay**: ESI's `retry_after` duration (typically 15 minutes)
//! - **Stagger offset**: Random 0-15 minute offset
//! - **Total delay**: `retry_after + random(0..900s)`
//! - **Max attempts**: 10 retries (same as exponential backoff)
//!
//! Without stagger offset, all rate-limited jobs would retry simultaneously,
//! immediately triggering another rate limit. The 15-minute stagger window
//! matches the initial job scheduler's distribution pattern.
//!
//! ## Permanent Failures
//!
//! Jobs fail permanently without retry for:
//! - 4xx client errors (invalid requests, programming bugs)
//! - Database query errors (constraint violations, bad queries)
//! - Configuration errors (missing/invalid environment variables)
//! - Parse errors (malformed data)
//! - Exceeding max retry attempts (10 attempts)
//!
//! # ESI Downtime Handling
//!
//! ESI has daily downtime from 11:00-11:05 UTC. The handler applies a 2-minute
//! grace period (10:58-11:07 UTC) and automatically reschedules jobs that fall
//! within this window to execute after downtime ends. Retry metadata is preserved
//! during downtime rescheduling.
//!
//! # Examples
//!
//! ## Successful Job Execution
//!
//! ```ignore
//! use bifrost::server::worker::handler::WorkerJobHandler;
//! use bifrost::server::model::worker::{WorkerJob, ScheduledWorkerJob};
//!
//! let handler = WorkerJobHandler::new(db, esi_provider, queue, true);
//! let job = ScheduledWorkerJob::new(
//!     WorkerJob::UpdateAllianceInfo { alliance_id: 123456 },
//!     Utc::now()
//! );
//!
//! // Job succeeds immediately
//! handler.handle(&job).await?;
//! ```
//!
//! ## Job with Transient Failure (Exponential Backoff)
//!
//! ```ignore
//! // First attempt fails with 500 error
//! handler.handle(&job).await?;
//! // -> Rescheduled with ~30s delay (±25% jitter)
//!
//! // Second attempt fails with 503 error
//! handler.handle(&job).await?;
//! // -> Rescheduled with ~60s delay (±25% jitter)
//!
//! // Third attempt succeeds
//! handler.handle(&job).await?;
//! // -> Job completes successfully
//! ```
//!
//! ## Job with Rate Limit (Staggered Retry)
//!
//! ```ignore
//! // Job fails with 429 rate limit (retry_after = 900s)
//! handler.handle(&job).await?;
//! // -> Rescheduled with 900s + random(0..900s) = 900-1800s total delay
//!
//! // Multiple jobs rate-limited simultaneously are distributed:
//! // - Job A: 900s + 47s  = 947s  (15m 47s)
//! // - Job B: 900s + 623s = 1523s (25m 23s)
//! // - Job C: 900s + 211s = 1111s (18m 31s)
//! // Prevents thundering herd when rate limit expires
//! ```
//!
//! ## Job with Permanent Failure
//!
//! ```ignore
//! // Job fails with 404 (entity not found)
//! let result = handler.handle(&job).await;
//! // -> Returns Err(AppError), job is NOT rescheduled
//! ```
//!
//! ## Job Exceeding Max Retries
//!
//! ```ignore
//! // Job fails 10 times over ~3.5 hours
//! let result = handler.handle(&job).await;
//! // -> Returns Err("Job exceeded maximum retry attempts")
//! // -> Job is permanently removed from queue
//! ```
mod eve;

use std::time::Duration;

use chrono::Utc;
use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;

use crate::server::{
    error::{retry::ErrorRetryStrategy, AppError},
    model::worker::{RetryMetadata, ScheduledWorkerJob, WorkerJob},
    service::eve::esi::EsiProvider,
    util::eve::get_esi_downtime_remaining,
    worker::queue::WorkerQueue,
};

/// Maximum number of retry attempts before permanently failing a job.
///
/// With exponential backoff starting at 30s and doubling each time:
/// - Attempt 1: 30s
/// - Attempt 2: 60s
/// - Attempt 3: 120s (2 min)
/// - Attempt 4: 240s (4 min)
/// - Attempt 5: 480s (8 min)
/// - Attempt 6: 960s (16 min)
/// - Attempt 7: 1800s (30 min, capped)
/// - Attempt 8: 1800s (30 min, capped)
/// - Attempt 9: 1800s (30 min, capped)
/// - Attempt 10: 1800s (30 min, capped)
///
/// Total time before permanent failure: ~3.5 hours
const MAX_RETRY_ATTEMPTS: u32 = 10;

/// Base delay for exponential backoff (30 seconds).
const BASE_RETRY_DELAY_SECS: u64 = 30;

/// Maximum backoff delay (30 minutes).
///
/// Caps the exponential backoff to prevent excessively long delays.
const MAX_RETRY_DELAY_SECS: u64 = 1800; // 30 minutes

/// Stagger window for rate-limited retries (15 minutes).
///
/// When multiple jobs are rate-limited simultaneously, they are distributed
/// across this window to prevent thundering herd when the rate limit expires.
/// This matches the stagger window used by the initial job scheduler.
const RATE_LIMIT_STAGGER_WINDOW_SECS: u64 = 900; // 15 minutes

/// Handler for processing worker jobs from the queue.
///
/// Provides a centralized interface for executing different types of worker jobs.
/// Each job type is dispatched to the appropriate service with logging and error handling.
pub struct WorkerJobHandler {
    db: DatabaseConnection,
    esi_provider: EsiProvider,
    queue: WorkerQueue,
    /// If true, checks for ESI downtime and reschedules jobs if within the downtime window.
    ///
    /// When enabled, the handler checks if the current time falls within ESI's daily downtime
    /// window (10:58-11:07 UTC) before processing jobs. If so, the job is rescheduled to run
    /// after downtime ends.
    ///
    /// Disable for testing to prevent time-dependent test failures.
    offset_for_esi_downtime: bool,
}

impl WorkerJobHandler {
    /// Creates a new WorkerJobHandler.
    ///
    /// Initializes a job handler with database, ESI provider with circuit breaker protection,
    /// ESI client for OAuth2, and worker queue.
    ///
    /// # Arguments
    /// - `db` - Database connection for data persistence
    /// - `esi_provider` - ESI provider with circuit breaker protection for data endpoints
    /// - `queue` - Worker queue for rescheduling jobs during downtime
    /// - `offset_for_esi_downtime` - If `true`, checks for ESI downtime and reschedules jobs.
    ///   Set to `false` for testing to prevent time-dependent failures.
    ///
    /// # Returns
    /// New job handler instance
    pub fn new(
        db: DatabaseConnection,
        esi_provider: EsiProvider,
        queue: WorkerQueue,
        offset_for_esi_downtime: bool,
    ) -> Self {
        Self {
            db,
            esi_provider,
            queue,
            offset_for_esi_downtime,
        }
    }

    /// Handles a worker job by delegating to the appropriate handler method.
    ///
    /// This is the main entry point for job processing. The handler:
    /// 1. Checks retry limits to prevent infinite loops
    /// 2. Checks for ESI downtime and reschedules if needed
    /// 3. Dispatches the job to the appropriate handler method
    /// 4. Handles failures with exponential backoff based on retry count
    ///
    /// # Retry Strategy
    ///
    /// The job is pushed back to the queue with exponential backoff based on the
    /// number of previous retry attempts. The backoff delay starts at 30 seconds and
    /// doubles with each retry, capped at 30 minutes.
    ///
    /// After `MAX_RETRY_ATTEMPTS` (10) retries, the job permanently fails to prevent
    /// infinite retry loops and memory leaks in Redis.
    ///
    /// # Arguments
    /// - `scheduled_job` - The worker job to execute with its scheduled timestamp
    ///
    /// # Returns
    /// - `Ok(())` - Job completed successfully, rescheduled due to downtime, or pushed back for retry
    /// - `Err(AppError)` - Job failed permanently (not retryable)
    pub async fn handle(&self, scheduled_job: &ScheduledWorkerJob) -> Result<(), AppError> {
        // Check if job has exceeded retry limit
        if let Some(metadata) = &scheduled_job.retry_metadata {
            if metadata.attempt_count >= MAX_RETRY_ATTEMPTS {
                tracing::error!(
                    "Job exceeded maximum retry attempts ({}) after {} total time. Failing permanently: {}. \
                    First failed at: {}",
                    MAX_RETRY_ATTEMPTS,
                    format_duration(Utc::now() - metadata.first_failed_at),
                    scheduled_job.job,
                    metadata.first_failed_at
                );
                return Err(AppError::Internal(
                    "Job exceeded maximum retry attempts".to_string(),
                ));
            }
        }

        if let Some(reschedule_time) = self.should_reschedule_for_downtime(scheduled_job) {
            self.queue
                .schedule(
                    scheduled_job.job.clone(),
                    reschedule_time,
                    scheduled_job.retry_metadata.clone(),
                )
                .await?;
            return Ok(());
        }

        let result = match &scheduled_job.job {
            WorkerJob::UpdateFactionInfo => self.update_faction_info().await,
            WorkerJob::UpdateAllianceInfo { alliance_id } => {
                self.update_alliance_info(*alliance_id).await
            }
            WorkerJob::UpdateCorporationInfo { corporation_id } => {
                self.update_corporation_info(*corporation_id).await
            }
            WorkerJob::UpdateCharacterInfo { character_id } => {
                self.update_character_info(*character_id).await
            }
            WorkerJob::UpdateAffiliations { character_ids } => {
                self.update_affiliations(character_ids.clone()).await
            }
        };

        let Err(e) = result else {
            return Ok(());
        };

        match e.to_retry_strategy() {
            ErrorRetryStrategy::Retry => {
                self.retry_job_with_backoff(scheduled_job, None).await?;
                Ok(())
            }
            ErrorRetryStrategy::RateLimited(retry_after) => {
                self.retry_job_with_backoff(scheduled_job, retry_after)
                    .await?;
                Ok(())
            }
            ErrorRetryStrategy::Fail => {
                tracing::error!(
                    "Job failed permanently (not retryable): {}. Error: {:?}",
                    scheduled_job.job,
                    e
                );

                Err(e)
            }
        }
    }

    /// Retries a job with exponential backoff based on retry count.
    ///
    /// Calculates the backoff delay using exponential backoff with jitter:
    /// - Base delay: 30 seconds
    /// - Multiplier: 2^attempt_count
    /// - Jitter: ±25% randomization to prevent thundering herd
    /// - Cap: 30 minutes maximum
    ///
    /// For rate-limited retries, adds a random offset within a 15-minute stagger window
    /// to prevent all rate-limited jobs from retrying simultaneously.
    ///
    /// # Arguments
    /// - `scheduled_job` - The job to retry
    /// - `override_delay` - Optional delay to use instead of exponential backoff
    ///   (e.g., for rate limiting with explicit retry_after)
    async fn retry_job_with_backoff(
        &self,
        scheduled_job: &ScheduledWorkerJob,
        override_delay: Option<Duration>,
    ) -> Result<(), AppError> {
        // Get or create retry metadata
        let mut metadata = scheduled_job
            .retry_metadata
            .clone()
            .unwrap_or_else(RetryMetadata::new);

        let backoff = if let Some(delay) = override_delay {
            // For rate-limited retries, add random stagger offset to prevent thundering herd
            let stagger_offset = calculate_stagger_offset();
            delay + stagger_offset
        } else {
            // Calculate exponential backoff with jitter
            calculate_backoff(metadata.attempt_count)
        };

        let reschedule_time = Utc::now() + chrono::Duration::from_std(backoff).unwrap();

        let log_message = if override_delay.is_some() {
            format!(
                "Job rate-limited (attempt {}/{}), pushing back to queue with {} second delay (includes stagger offset): {}. \
                Time since first failure: {}",
                metadata.attempt_count + 1,
                MAX_RETRY_ATTEMPTS,
                backoff.as_secs(),
                scheduled_job.job,
                format_duration(Utc::now() - metadata.first_failed_at),
            )
        } else {
            format!(
                "Job failed after internal retries (attempt {}/{}), pushing back to queue with {} second backoff: {}. \
                Time since first failure: {}",
                metadata.attempt_count + 1,
                MAX_RETRY_ATTEMPTS,
                backoff.as_secs(),
                scheduled_job.job,
                format_duration(Utc::now() - metadata.first_failed_at),
            )
        };

        tracing::warn!("{}", log_message);

        // Increment retry count for next attempt
        metadata.increment();

        // Schedule with retry metadata
        self.queue
            .schedule(scheduled_job.job.clone(), reschedule_time, Some(metadata))
            .await?;

        Ok(())
    }

    /// Checks if we're in ESI downtime and if the job should be rescheduled.
    ///
    /// Logs appropriate messages based on when the job was scheduled relative to
    /// the downtime window.
    ///
    /// # Arguments
    /// - `scheduled_job` - The worker job being evaluated
    ///
    /// # Returns
    /// - `Some(reschedule_time)` - Job should be rescheduled to this time
    /// - `None` - No downtime detected or check is disabled, proceed with job
    fn should_reschedule_for_downtime(
        &self,
        scheduled_job: &ScheduledWorkerJob,
    ) -> Option<chrono::DateTime<Utc>> {
        if !self.offset_for_esi_downtime {
            return None;
        }

        let now = Utc::now();
        let downtime_remaining = get_esi_downtime_remaining(now)?;

        // Check if job was scheduled before downtime window started
        // Downtime window is 11:00-11:05 UTC (with 2 minute grace period surrounding the window)
        let downtime_start = now - downtime_remaining;

        if scheduled_job.scheduled_at < downtime_start {
            tracing::debug!(
                "Job scheduled at {} (before downtime window) pulled during ESI downtime. \
                Rescheduling to run after downtime ends (in {} minutes): {}\n\
                This behavior is expected when the application restarts during ESI downtime.",
                scheduled_job.scheduled_at,
                downtime_remaining.num_minutes(),
                scheduled_job.job
            );
        } else {
            tracing::warn!(
                "Job scheduled at {} (during downtime window starting at {}) is being processed during ESI downtime. \
                Rescheduling to run after downtime ends (in {} minutes): {}\n\
                This may indicate a scheduler bug. Please open a GitHub issue if this behavior continues.",
                scheduled_job.scheduled_at,
                downtime_start,
                downtime_remaining.num_minutes(),
                scheduled_job.job
            );
        }

        Some(now + downtime_remaining)
    }
}

/// Calculates exponential backoff delay with jitter.
///
/// Formula: base * 2^attempt * jitter_factor
/// - Base: 30 seconds
/// - Jitter: ±25% randomization
/// - Cap: 30 minutes
fn calculate_backoff(attempt_count: u32) -> Duration {
    use rand::Rng;

    // Calculate base exponential backoff: base * 2^attempt
    let base_delay = BASE_RETRY_DELAY_SECS * 2_u64.pow(attempt_count);

    // Cap at maximum delay
    let capped_delay = base_delay.min(MAX_RETRY_DELAY_SECS);

    // Add jitter: ±25% randomization to prevent thundering herd
    let mut rng = rand::rng();
    let jitter_factor = rng.random_range(0.75..=1.25);
    let jittered_delay = (capped_delay as f64 * jitter_factor) as u64;

    Duration::from_secs(jittered_delay)
}

/// Calculates a random stagger offset for rate-limited retries.
///
/// Returns a random duration within the stagger window to distribute rate-limited
/// jobs across time, preventing thundering herd when the rate limit expires.
///
/// # Returns
/// Random duration between 0 and 15 minutes
fn calculate_stagger_offset() -> Duration {
    use rand::Rng;

    let mut rng = rand::rng();
    let offset_secs = rng.random_range(0..=RATE_LIMIT_STAGGER_WINDOW_SECS);
    Duration::from_secs(offset_secs)
}

/// Formats a duration in a human-readable format.
fn format_duration(duration: chrono::Duration) -> String {
    let total_secs = duration.num_seconds();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}
