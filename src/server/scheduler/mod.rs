//! Scheduler for periodic EVE Online data refresh tasks.
//!
//! This module provides a cron-based job scheduler that automatically refreshes cached EVE Online
//! entity data (factions, alliances, corporations, characters, and affiliations) by dispatching
//! worker queue jobs at configured intervals. The scheduler ensures data remains fresh according
//! to ESI cache expiration times while distributing load evenly across refresh windows.

use std::future::Future;
use std::sync::Arc;

use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::server::{error::Error, worker::WorkerQueue};

pub mod config;
pub mod entity_refresh;
pub mod eve;
pub mod schedule;

#[cfg(test)]
mod tests;

use self::eve::{
    affiliation::schedule_character_affiliation_update, alliance::schedule_alliance_info_update,
    character::schedule_character_info_update, corporation::schedule_corporation_info_update,
    faction::schedule_faction_info_update,
};

use self::config::eve::{
    alliance as alliance_config, character as character_config,
    character_affiliation as character_affiliation_config, corporation as corporation_config,
    faction as faction_config,
};

/// Shared state for scheduler operations and entity refresh tracking.
///
/// Contains the database connection, worker queue, and configuration flags used across
/// the scheduler system. This struct is designed to be cheaply cloneable since both
/// `DatabaseConnection` and `WorkerQueue` contain `Arc` internally, making it suitable
/// for passing into `'static` closures required by the cron job scheduler.
///
/// # Fields
/// - `db` - Database connection for querying entities that need cache refresh
/// - `queue` - Worker queue for dispatching asynchronous refresh tasks
/// - `offset_for_esi_downtime` - Controls whether job scheduling accounts for ESI downtime
///
/// # Usage
/// `SchedulerState` is passed to EVE entity scheduling functions (like `schedule_alliance_info_update`)
/// and can be borrowed by `EntityRefreshTracker` to access scheduler configuration while
/// calculating batch limits and creating job schedules.
///
/// # Cloning
/// Cloning `SchedulerState` is cheap (O(1)) because it only increments reference counts
/// for the underlying `Arc`-wrapped database and queue resources.
#[derive(Clone)]
pub struct SchedulerState {
    /// DDatabase connection for querying entities that need cache refresh
    pub db: DatabaseConnection,
    /// Worker queue for dispatching asynchronous refresh tasks
    pub queue: WorkerQueue,
    /// If true, offsets scheduled jobs and batch size to avoid overlapping with ESI downtime.
    ///
    /// When enabled, the scheduler accounts for ESI's daily downtime window (10:58-11:07 UTC)
    /// by adjusting batch sizes and shifting job execution times to avoid the downtime period.
    ///
    /// Disable for testing to prevent flaky tests that could fail if executed during or near
    /// the ESI downtime window when using `Utc::now()` for time calculations.
    pub offset_for_esi_downtime: bool,
}

///
/// The scheduler manages periodic jobs that keep EVE-related entities (characters, corporations,
/// alliances, factions, and affiliations) up-to-date by scheduling worker queue tasks according
/// to configured cron expressions.
pub struct Scheduler {
    state: SchedulerState,
    sched: JobScheduler,
}

impl Scheduler {
    /// Creates a new instance of [`Scheduler`].
    ///
    /// Initializes the underlying `JobScheduler` and prepares the scheduler with the provided
    /// database connection and worker queue.
    ///
    /// # Arguments
    /// - `db` - Database connection for querying entities that need updates
    /// - `queue` - Worker queue for dispatching asynchronous refresh tasks
    /// - `offset_for_esi_downtime` - If `true`, adjusts job scheduling to avoid ESI downtime
    ///   window (10:58-11:07 UTC). Set to `false` for testing to prevent time-dependent failures.
    ///
    /// # Returns
    /// - `Ok(Scheduler)` - Successfully created scheduler instance
    /// - `Err(Error)` - Failed to initialize the underlying job scheduler
    pub async fn new(
        db: DatabaseConnection,
        queue: WorkerQueue,
        offset_for_esi_downtime: bool,
    ) -> Result<Self, Error> {
        let sched = JobScheduler::new().await?;
        let state = SchedulerState {
            db,
            queue,
            offset_for_esi_downtime,
        };

        Ok(Self { state, sched })
    }

    /// Registers all scheduled jobs and starts the scheduler.
    ///
    /// This method configures and registers all EVE Online data refresh jobs with their respective
    /// cron schedules, then starts the scheduler to begin executing jobs. Once started, jobs will
    /// run automatically according to their configured cron expressions until the scheduler is stopped.
    ///
    /// The following jobs are registered:
    /// - Faction info updates
    /// - Alliance info updates
    /// - Corporation info updates
    /// - Character info updates
    /// - Character affiliation updates
    ///
    /// # Returns
    /// - `Ok(())` - All jobs successfully registered and scheduler started
    /// - `Err(Error)` - Failed to register a job or start the scheduler
    pub async fn start(mut self) -> Result<(), Error> {
        self.schedule_job(
            faction_config::CRON_EXPRESSION,
            "faction info",
            schedule_faction_info_update,
        )
        .await?;

        self.schedule_job(
            alliance_config::CRON_EXPRESSION,
            "alliance info",
            schedule_alliance_info_update,
        )
        .await?;

        self.schedule_job(
            corporation_config::CRON_EXPRESSION,
            "corporation info",
            schedule_corporation_info_update,
        )
        .await?;

        self.schedule_job(
            character_config::CRON_EXPRESSION,
            "character info",
            schedule_character_info_update,
        )
        .await?;

        self.schedule_job(
            character_affiliation_config::CRON_EXPRESSION,
            "character affiliation",
            schedule_character_affiliation_update,
        )
        .await?;

        // Start the scheduler
        self.sched.start().await?;

        Ok(())
    }

    /// Schedules a recurring job with the specified cron expression.
    ///
    /// Registers a new asynchronous job with the scheduler that executes the provided function
    /// according to the cron expression. The function receives a clone of `SchedulerState`,
    /// which contains the database connection and worker queue, allowing it to query for
    /// entities and dispatch refresh tasks.
    ///
    /// On execution, the job logs the number of updates scheduled (on success) or any errors
    /// that occur during scheduling.
    ///
    /// # Arguments
    /// - `cron` - Cron expression defining when the job should run (e.g., "0 0 * * * *" for hourly)
    /// - `name` - Human-readable name for the job (used in log messages)
    /// - `function` - Async function that receives `SchedulerState`, queries entities, and schedules
    ///   updates, returning the count of scheduled tasks
    ///
    /// # Returns
    /// - `Ok(())` - Job successfully registered with the scheduler
    /// - `Err(Error)` - Failed to create or add the job (invalid cron expression or scheduler error)
    pub async fn schedule_job<F, Fut>(
        &mut self,
        cron: &str,
        name: &str,
        function: F,
    ) -> Result<(), Error>
    where
        F: Fn(SchedulerState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<usize, Error>> + Send + 'static,
    {
        let state = self.state.clone();
        let name = name.to_string();
        let function = Arc::new(function);

        self.sched
            .add(Job::new_async(cron, move |_, _| {
                let state = state.clone();
                let name = name.clone();
                let function = Arc::clone(&function);

                Box::pin(async move {
                    match function(state).await {
                        Ok(count) => tracing::debug!("Scheduled {} {} update(s)", count, name),
                        Err(e) => tracing::error!("Error scheduling {} update: {:?}", name, e),
                    }
                })
            })?)
            .await?;

        Ok(())
    }
}
