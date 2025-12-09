//! Worker job handler for processing background tasks.
//!
//! This module provides the `WorkerJobHandler` that executes different types of worker
//! jobs including EVE Online data updates. Each job type is dispatched to the appropriate
//! service method with error handling and logging.

use chrono::Utc;
use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error,
    model::worker::{ScheduledWorkerJob, WorkerJob},
    service::eve::{
        affiliation::AffiliationService, alliance::AllianceService, character::CharacterService,
        corporation::CorporationService, faction::FactionService,
    },
    util::eve::{get_esi_downtime_remaining, ESI_AFFILIATION_REQUEST_LIMIT},
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
    /// # Arguments
    /// - `scheduled_job` - The worker job to execute with its scheduled timestamp
    ///
    /// # Returns
    /// - `Ok(())` - Job completed successfully or rescheduled due to downtime
    /// - `Err(Error)` - Job failed with error (logged automatically by each handler)
    pub async fn handle(&self, scheduled_job: &ScheduledWorkerJob) -> Result<(), Error> {
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
        match &scheduled_job.job {
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
        }
    }

    /// Updates NPC faction information from ESI.
    ///
    /// Checks if the faction cache has expired and fetches updated faction data from ESI
    /// if needed. ESI caches faction data for 24 hours, so this may not fetch new data
    /// on every call.
    ///
    /// # Returns
    /// - `Ok(())` - Faction update completed (or skipped if cache valid)
    /// - `Err(Error)` - Failed to update factions
    pub async fn update_faction_info(&self) -> Result<(), Error> {
        tracing::debug!("Checking for daily NPC faction info update");

        let factions = FactionService::new(&self.db, &self.esi_client)
            .update_factions()
            .await
            .map_err(|e| {
                tracing::error!("Failed to update NPC faction information: {:?}", e);
                e
            })?;

        if factions.is_empty() {
            tracing::debug!("NPC faction information already up to date, no update needed");
        } else {
            tracing::debug!(
                "Successfully updated NPC faction information for {} factions",
                factions.len()
            );
        }

        Ok(())
    }

    /// Updates alliance information from ESI.
    ///
    /// Fetches alliance data from ESI and persists it to the database. If the alliance
    /// has faction affiliations, those dependencies are resolved first.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID to update
    ///
    /// # Returns
    /// - `Ok(())` - Alliance info updated successfully
    /// - `Err(Error)` - Failed to fetch or persist alliance data
    pub async fn update_alliance_info(&self, alliance_id: i64) -> Result<(), Error> {
        tracing::debug!(
            "Processing alliance info update for alliance_id: {}",
            alliance_id
        );

        AllianceService::new(&self.db, &self.esi_client)
            .upsert(alliance_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to update info for alliance {}: {:?}",
                    alliance_id,
                    e
                );
                e
            })?;

        tracing::debug!("Successfully updated info for alliance {}", alliance_id);

        Ok(())
    }

    /// Updates corporation information from ESI.
    ///
    /// Fetches corporation data from ESI and persists it to the database. If the corporation
    /// has alliance or faction affiliations, those dependencies are resolved first.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID to update
    ///
    /// # Returns
    /// - `Ok(())` - Corporation info updated successfully
    /// - `Err(Error)` - Failed to fetch or persist corporation data
    pub async fn update_corporation_info(&self, corporation_id: i64) -> Result<(), Error> {
        tracing::debug!(
            "Processing corporation info update for corporation_id: {}",
            corporation_id
        );

        CorporationService::new(&self.db, &self.esi_client)
            .upsert(corporation_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to update info for corporation {}: {:?}",
                    corporation_id,
                    e
                );
                e
            })?;

        tracing::debug!(
            "Successfully updated info for corporation {}",
            corporation_id
        );

        Ok(())
    }

    /// Updates character information from ESI.
    ///
    /// Fetches character data from ESI and persists it to the database. If the character
    /// has corporation or faction affiliations, those dependencies are resolved first.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID to update
    ///
    /// # Returns
    /// - `Ok(())` - Character info updated successfully
    /// - `Err(Error)` - Failed to fetch or persist character data
    pub async fn update_character_info(&self, character_id: i64) -> Result<(), Error> {
        tracing::debug!(
            "Processing character info update for character_id: {}",
            character_id
        );

        CharacterService::new(&self.db, &self.esi_client)
            .upsert(character_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to update info for character {}: {:?}",
                    character_id,
                    e
                );
                e
            })?;

        tracing::debug!("Successfully updated info for character {}", character_id);

        Ok(())
    }

    /// Updates affiliations for multiple characters in bulk.
    ///
    /// Fetches character affiliation data from ESI and updates both character-to-corporation
    /// and corporation-to-alliance relationships. Validates the character ID list and
    /// truncates to ESI's limit of 1000 characters if necessary.
    ///
    /// # Arguments
    /// - `character_ids` - List of EVE Online character IDs to update affiliations for
    ///
    /// # Returns
    /// - `Ok(())` - Affiliations updated successfully
    /// - `Err(Error)` - Failed to fetch or persist affiliation data
    pub async fn update_affiliations(&self, character_ids: Vec<i64>) -> Result<(), Error> {
        let count = character_ids.len();
        tracing::debug!("Processing affiliations update for {} characters", count);

        if character_ids.is_empty() {
            tracing::debug!("No characters to update affiliations for");
            return Ok(());
        }

        if character_ids.len() > ESI_AFFILIATION_REQUEST_LIMIT {
            tracing::warn!(
                "Update affiliation job contains {} character IDs, exceeding ESI affiliation request limit of {}; truncating to limit",
                character_ids.len(),
                ESI_AFFILIATION_REQUEST_LIMIT
            );
        }

        AffiliationService::new(&self.db, &self.esi_client)
            .update_affiliations(character_ids)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update affiliations due to error: {:?}", e);
                e
            })?;

        tracing::debug!("Successfully updated affiliations for {} characters", count);

        Ok(())
    }
}
