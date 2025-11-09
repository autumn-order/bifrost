use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerJob {
    UpdateAllianceInfo { alliance_id: i64 },
    UpdateCorporationInfo { corporation_id: i64 },
    UpdateCharacterInfo { character_id: i64 },
    UpdateAffiliations { count: u64 },
}

impl WorkerJob {
    // While the job scheduler schedules within 30 minute windows, it may be possible
    // that some jobs extend beyond that window in the instance of:
    // - A high queue volume causing jobs to pile up past the window
    // - The application going offline and coming back later, restoring the Redis queue
    //   but scheduling new jobs despite all the currently scheduled jobs having not been completed.
    //
    // Tracking keys are used to determine what jobs of each category may still be present in the
    // queue to prevent the accidental scheduling of duplicate jobs.
    //
    // Lifecycle:
    // 1. Scheduler atomically sets tracking key with SET NX (set if not exists) before scheduling job
    // 2. Key has TTL of 2x schedule_interval as safety margin
    // 3. Worker deletes key immediately upon job completion (see worker::handle_job)
    // 4. If worker crashes, TTL ensures key doesn't block forever

    /// Generate a Redis key for tracking pending jobs
    ///
    /// These keys are used to prevent duplicate job scheduling via atomic SET NX operations.
    /// Workers are responsible for deleting these keys upon job completion.
    pub fn tracking_key(&self) -> String {
        match self {
            WorkerJob::UpdateCharacterInfo { character_id } => {
                format!("job:pending:character:info:{}", character_id)
            }
            WorkerJob::UpdateAllianceInfo { alliance_id } => {
                format!("job:pending:alliance:info:{}", alliance_id)
            }
            WorkerJob::UpdateCorporationInfo { corporation_id } => {
                format!("job:pending:corporation:info:{}", corporation_id)
            }
            WorkerJob::UpdateAffiliations { .. } => {
                // Single key for batch job
                //
                // If there are any affiliation update tracking keys in redis we
                // won't schedule another affiliation job yet
                "job:pending:affiliation:batch".to_string()
            }
        }
    }
}
