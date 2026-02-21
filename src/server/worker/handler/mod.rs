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
    /// - `WorkerJobHandler` - New job handler instance
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
    /// This is the main entry point for job processing. If ESI downtime offset is enabled,
    /// checks if the current time falls within ESI's daily downtime window (11:00-11:05 UTC + 2 minute
    /// grace period surrounding the window). If currently in downtime, checks the job's scheduled
    /// timestamp to determine if:
    /// - Job was scheduled before downtime (app restart case) - reschedule with info log
    /// - Job was scheduled during downtime (scheduler bug) - reschedule with warning
    ///
    /// Otherwise, pattern matches on the job type and dispatches to the corresponding handler
    /// method. Each handler method logs the operation and handles errors appropriately.
    ///
    /// If a job fails and the error retry strategy indicates it should be retried, the job
    /// is pushed back to the queue with a backoff delay. Rate-limited errors
    /// (429) use the retry_after duration if provided. Jobs are retried up to 3 times
    /// internally within each handler method (via RetryContext), so queue-level retries
    /// handle cases where all internal attempts have been exhausted.
    ///
    /// # Arguments
    /// - `scheduled_job` - The worker job to execute with its scheduled timestamp
    ///
    /// # Returns
    /// - `Ok(())` - Job completed successfully, rescheduled due to downtime, or pushed back for retry
    /// - `Err(AppError)` - Job failed permanently (not retryable)
    pub async fn handle(&self, scheduled_job: &ScheduledWorkerJob) -> Result<(), AppError> {
        // Check if we're within ESI downtime window (if offset is enabled)
        if self.offset_for_esi_downtime {
            let now = Utc::now();
            if let Some(downtime_remaining) = get_esi_downtime_remaining(now) {
                // Check if job was scheduled before downtime window started
                // Downtime window is 11:00-11:05 UTC (with 2 minute grace period surrounding the window)
                let downtime_start = now - downtime_remaining;

                if scheduled_job.scheduled_at < downtime_start {
                    // Job was scheduled before downtime - likely app restart during downtime
                    tracing::debug!(
                        "Job scheduled at {} (before downtime window) pulled during ESI downtime. \
                        Rescheduling to run after downtime ends (in {} minutes): {}\n\
                        This behavior is expected when the application restarts during ESI downtime.",
                        scheduled_job.scheduled_at,
                        downtime_remaining.num_minutes(),
                        scheduled_job.job
                    );
                } else {
                    // Job was scheduled within downtime window - scheduler bug
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

                // Calculate when downtime ends
                let reschedule_time = now + downtime_remaining;

                // Reschedule the job to run after downtime
                self.queue
                    .schedule(scheduled_job.job.clone(), reschedule_time)
                    .await?;

                return Ok(());
            }
        }

        // Process the job normally
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

        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                match e.to_retry_strategy() {
                    ErrorRetryStrategy::Retry => {
                        let backoff = Duration::from_secs(120); // 2 minute basic backoff
                        let reschedule_time =
                            Utc::now() + chrono::Duration::from_std(backoff).unwrap();

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
                        let reschedule_time =
                            Utc::now() + chrono::Duration::from_std(backoff).unwrap();

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
        }
    }
}
