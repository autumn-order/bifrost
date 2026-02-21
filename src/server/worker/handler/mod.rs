//! Worker job handler for processing background tasks.
//!
//! This module provides the `WorkerJobHandler` that executes different types of worker
//! jobs including EVE Online data updates. Each job type is dispatched to the appropriate
//! service method with error handling and logging.
mod eve;

use std::time::Duration;

use chrono::Utc;
use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;

use crate::server::{
    error::{retry::ErrorRetryStrategy, AppError},
    model::worker::{ScheduledWorkerJob, WorkerJob},
    util::eve::get_esi_downtime_remaining,
    worker::queue::WorkerQueue,
};

/// Handler for processing worker jobs from the queue.
///
/// Provides a centralized interface for executing different types of worker jobs.
/// Each job type is dispatched to the appropriate service with logging and error handling.
pub struct WorkerJobHandler {
    db: DatabaseConnection,
    esi_client: eve_esi::Client,
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
    /// Initializes a job handler with database, ESI client connections, and worker queue.
    ///
    /// # Arguments
    /// - `db` - Database connection for data persistence
    /// - `esi_client` - ESI API client for fetching EVE Online data
    /// - `queue` - Worker queue for rescheduling jobs during downtime
    /// - `offset_for_esi_downtime` - If `true`, checks for ESI downtime and reschedules jobs.
    ///   Set to `false` for testing to prevent time-dependent failures.
    ///
    /// # Returns
    /// New job handler instance
    pub fn new(
        db: DatabaseConnection,
        esi_client: eve_esi::Client,
        queue: WorkerQueue,
        offset_for_esi_downtime: bool,
    ) -> Self {
        Self {
            db,
            esi_client,
            queue,
            offset_for_esi_downtime,
        }
    }

    /// Handles a worker job by delegating to the appropriate handler method.
    ///
    /// This is the main entry point for job processing. The handler:
    /// 1. Checks for ESI downtime and reschedules if needed
    /// 2. Dispatches the job to the appropriate handler method
    /// 3. Handles failures based on retry strategy (retry, rate-limited, or fail permanently)
    ///
    /// Jobs are retried internally up to 3 times (via RetryContext). If all internal attempts
    /// are exhausted, the job is pushed back to the queue with a backoff delay based on the
    /// error type.
    ///
    /// # Arguments
    /// - `scheduled_job` - The worker job to execute with its scheduled timestamp
    ///
    /// # Returns
    /// - `Ok(())` - Job completed successfully, rescheduled due to downtime, or pushed back for retry
    /// - `Err(AppError)` - Job failed permanently (not retryable)
    pub async fn handle(&self, scheduled_job: &ScheduledWorkerJob) -> Result<(), AppError> {
        if let Some(reschedule_time) = self.should_reschedule_for_downtime(scheduled_job) {
            self.queue
                .schedule(scheduled_job.job.clone(), reschedule_time)
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
                let backoff = Duration::from_secs(120); // 2 minute basic backoff
                let reschedule_time = Utc::now() + chrono::Duration::from_std(backoff).unwrap();

                tracing::warn!(
                    "Job failed after internal retries, pushing back to queue with {} second backoff: {}. Error: {:?}",
                    backoff.as_secs(),
                    scheduled_job.job,
                    e
                );

                self.queue
                    .schedule(scheduled_job.job.clone(), reschedule_time)
                    .await?;

                Ok(())
            }
            ErrorRetryStrategy::RateLimited(retry_after) => {
                let backoff = retry_after.unwrap_or_else(|| Duration::from_secs(900));
                let reschedule_time = Utc::now() + chrono::Duration::from_std(backoff).unwrap();

                tracing::warn!(
                    "Job ESI rate limited (429), pushing back to queue with {} second backoff: {}",
                    backoff.as_secs(),
                    scheduled_job.job
                );

                self.queue
                    .schedule(scheduled_job.job.clone(), reschedule_time)
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
