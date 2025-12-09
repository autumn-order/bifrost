//! Scheduler for periodic EVE Online data refresh tasks.
//!
//! This module provides a cron-based job scheduler that automatically refreshes cached EVE Online
//! entity data (factions, alliances, corporations, characters, and affiliations) by dispatching
//! worker queue jobs at configured intervals. The scheduler ensures data remains fresh according
//! to ESI cache expiration times while distributing load evenly across refresh windows.

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

/// Job scheduler for managing background EVE Online data refresh tasks.
///
/// The scheduler manages periodic jobs that keep EVE-related entities (characters, corporations,
/// alliances, factions, and affiliations) up-to-date by scheduling worker queue tasks according
/// to configured cron expressions.
pub struct Scheduler {
    db: DatabaseConnection,
    queue: WorkerQueue,
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
    ///
    /// # Returns
    /// - `Ok(Scheduler)` - Successfully created scheduler instance
    /// - `Err(Error)` - Failed to initialize the underlying job scheduler
    pub async fn new(db: DatabaseConnection, queue: WorkerQueue) -> Result<Self, Error> {
        let sched = JobScheduler::new().await?;
        Ok(Self { db, queue, sched })
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
    /// according to the cron expression. The function receives clones of the database connection
    /// and worker queue, allowing it to query for entities and dispatch refresh tasks.
    ///
    /// On execution, the job logs the number of updates scheduled (on success) or any errors
    /// that occur during scheduling.
    ///
    /// # Arguments
    /// - `cron` - Cron expression defining when the job should run (e.g., "0 0 * * * *" for hourly)
    /// - `name` - Human-readable name for the job (used in log messages)
    /// - `function` - Async function that queries entities and schedules updates, returning the count of scheduled tasks
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
        F: Fn(DatabaseConnection, WorkerQueue) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<usize, Error>> + Send + 'static,
    {
        let db = self.db.clone();
        let queue = self.queue.clone();
        let name = name.to_string();
        let function = Arc::new(function);

        self.sched
            .add(Job::new_async(cron, move |_, _| {
                let db = db.clone();
                let queue = queue.clone();
                let name = name.clone();
                let function = Arc::clone(&function);

                Box::pin(async move {
                    match function(db, queue).await {
                        Ok(count) => tracing::debug!("Scheduled {} {} update(s)", count, name),
                        Err(e) => tracing::error!("Error scheduling {} update: {:?}", name, e),
                    }
                })
            })?)
            .await?;

        Ok(())
    }
}
