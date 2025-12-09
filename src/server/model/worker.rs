//! Worker job definitions for background task processing.
//!
//! This module defines the `WorkerJob` enum representing all types of background jobs that
//! can be dispatched to the worker queue. Jobs are serialized to JSON for Redis storage and
//! deserialized by worker handlers for processing. Each job variant contains the minimal
//! data needed to perform the task (e.g., entity IDs to refresh).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Worker job with scheduled execution timestamp.
///
/// Wraps a `WorkerJob` with the timestamp it was scheduled for execution. This allows
/// the worker handler to distinguish between jobs scheduled before ESI downtime (which
/// should be rescheduled) versus jobs scheduled during downtime (scheduler bug).
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledWorkerJob {
    /// The worker job to execute.
    pub job: WorkerJob,
    /// The UTC timestamp when this job was scheduled for execution.
    pub scheduled_at: DateTime<Utc>,
}

impl ScheduledWorkerJob {
    /// Creates a new scheduled worker job.
    ///
    /// # Arguments
    /// - `job` - The worker job to execute
    /// - `scheduled_at` - The UTC timestamp when the job was scheduled
    ///
    /// # Returns
    /// - `ScheduledWorkerJob` - New scheduled job instance
    pub fn new(job: WorkerJob, scheduled_at: DateTime<Utc>) -> Self {
        Self { job, scheduled_at }
    }
}

impl fmt::Display for ScheduledWorkerJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (scheduled at {})", self.job, self.scheduled_at)
    }
}

/// Background job types for EVE Online data refresh operations.
///
/// Each variant represents a specific type of background task that can be enqueued to the
/// Redis-backed worker queue. Jobs are serialized to JSON for storage and deserialized by
/// worker handlers for execution. The scheduler and services create these jobs to refresh
/// cached EVE Online entity data when it expires.
///
/// # Job Types
/// - `UpdateFactionInfo` - Refresh all NPC faction data from ESI
/// - `UpdateAllianceInfo` - Refresh specific alliance metadata
/// - `UpdateCorporationInfo` - Refresh specific corporation metadata
/// - `UpdateCharacterInfo` - Refresh specific character metadata
/// - `UpdateAffiliations` - Refresh corporation/alliance affiliations for multiple characters (batched)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkerJob {
    /// Update NPC faction information for all factions.
    ///
    /// This job checks if the faction cache has expired (24-hour TTL) and if so, fetches
    /// updated faction data from ESI and persists it to the database. Only a single faction
    /// update job is needed since there are a small, fixed number of NPC factions in EVE Online.
    UpdateFactionInfo,

    /// Update information for a specific alliance.
    ///
    /// Fetches fresh alliance metadata (name, ticker, executor corporation, etc.) from ESI
    /// and updates the database record. Used to keep alliance information current with a
    /// 24-hour cache duration.
    ///
    /// # Fields
    /// - `alliance_id` - EVE Online alliance ID to refresh
    UpdateAllianceInfo {
        /// EVE Online alliance ID to refresh.
        alliance_id: i64,
    },

    /// Update information for a specific corporation.
    ///
    /// Fetches fresh corporation metadata (name, ticker, alliance membership, CEO, etc.) from
    /// ESI and updates the database record. Used to keep corporation information current with
    /// a 24-hour cache duration.
    ///
    /// # Fields
    /// - `corporation_id` - EVE Online corporation ID to refresh
    UpdateCorporationInfo {
        /// EVE Online corporation ID to refresh.
        corporation_id: i64,
    },

    /// Update information for a specific character.
    ///
    /// Fetches fresh character metadata (name, birthday, description, etc.) from ESI and
    /// updates the database record. Used to keep character information current with a 30-day
    /// cache duration, as character info rarely change.
    ///
    /// # Fields
    /// - `character_id` - EVE Online character ID to refresh
    UpdateCharacterInfo {
        /// EVE Online character ID to refresh.
        character_id: i64,
    },

    /// Update affiliations (corporation/alliance/faction) for multiple characters.
    ///
    /// Fetches current affiliation data for a batch of characters from ESI's bulk affiliation
    /// endpoint and updates the database. Affiliations change when characters join/leave
    /// corporations or when corporations join/leave alliances. Batched to efficiently use
    /// ESI's bulk endpoint (up to 1000 characters per request) with a 1-hour cache duration.
    ///
    /// # Fields
    /// - `character_ids` - List of EVE Online character IDs to refresh (max 1000 per ESI limit)
    UpdateAffiliations {
        /// List of EVE Online character IDs to refresh (max 1000 per ESI limit).
        character_ids: Vec<i64>,
    },
}

/// Custom Display implementation for readable job logging.
///
/// Provides human-readable string representations of worker jobs for logging and debugging.
/// For `UpdateAffiliations` jobs with many character IDs, the display format is condensed to
/// show only a sample of IDs (first 3 and last 2) plus the total count to avoid cluttering
/// logs with potentially hundreds of IDs. Other job variants use their Debug representation.
///
/// # Examples
/// ```ignore
/// // Small batch (≤5 characters) - shows all IDs
/// UpdateAffiliations { character_ids: [123, 456, 789] }
///
/// // Large batch (>5 characters) - shows sample and count
/// UpdateAffiliations { character_ids: [123, 456, 789, ..., 998, 999] (200 total) }
///
/// // Other job types - uses Debug format
/// UpdateAllianceInfo { alliance_id: 123456 }
/// ```
impl fmt::Display for WorkerJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkerJob::UpdateAffiliations { character_ids } => {
                let count = character_ids.len();
                if count <= 5 {
                    // Show all IDs if there are 5 or fewer
                    write!(
                        f,
                        "UpdateAffiliations {{ character_ids: {:?} }}",
                        character_ids
                    )
                } else {
                    // Show first 3 and last 2 IDs with count for larger batches
                    write!(
                        f,
                        "UpdateAffiliations {{ character_ids: [{}, {}, {}, ..., {}, {}] ({} total) }}",
                        character_ids[0],
                        character_ids[1],
                        character_ids[2],
                        character_ids[count - 2],
                        character_ids[count - 1],
                        count
                    )
                }
            }
            // Use Debug formatting for all other variants
            other => write!(f, "{:?}", other),
        }
    }
}
