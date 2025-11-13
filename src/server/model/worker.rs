use serde::{Deserialize, Serialize};

use crate::server::error::{worker::WorkerError, Error};

/// Maximum number of character IDs allowed in a single UpdateAffiliations job
/// This matches the EVE ESI API limit for affiliation lookups
pub const MAX_AFFILIATION_BATCH_SIZE: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerJob {
    UpdateAllianceInfo { alliance_id: i64 },
    UpdateCorporationInfo { corporation_id: i64 },
    UpdateCharacterInfo { character_id: i64 },
    // TODO: change to count for new hash approach for affiliation identity
    UpdateAffiliations { character_ids: Vec<i64> },
}

impl WorkerJob {
    /// Generate a Redis key for tracking pending jobs
    ///
    /// These keys are used to prevent duplicate job scheduling via atomic SET NX operations.
    /// Workers are responsible for deleting these keys upon job completion.
    ///
    /// WARNING: Will be replaced with identity methods, do not use
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
            WorkerJob::UpdateAffiliations { character_ids } => {
                // Generate unique key per batch based on the character IDs in the job
                //
                // We use a hash of the sorted character IDs to create a stable identifier
                // that's unique per batch but doesn't grow linearly with batch size.
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let mut sorted_ids = character_ids.clone();
                sorted_ids.sort_unstable();

                let mut hasher = DefaultHasher::new();
                sorted_ids.hash(&mut hasher);
                let hash = hasher.finish();

                format!("job:pending:affiliation:batch:{:x}", hash)
            }
        }
    }

    /// Get identity for worker job
    ///
    /// Returns a unique identifier for the job. For affiliation batches, uses a hash
    /// and count instead of storing all character IDs to keep the identity string compact.
    /// The actual character IDs must be retrieved from the database using the count.
    ///
    /// # Errors
    ///
    /// Returns an error if UpdateAffiliations contains more than MAX_AFFILIATION_BATCH_SIZE IDs.
    ///
    /// # Examples
    ///
    /// ```
    /// // Character job: "character:info:123456"
    /// // Alliance job: "alliance:info:789012"
    /// // Corporation job: "corporation:info:345678"
    /// // Affiliations job: "affiliation:batch:100:a1b2c3d4e5f6g7h8" (count:hash)
    /// ```
    pub fn identity(&self) -> Result<String, Error> {
        match self {
            WorkerJob::UpdateCharacterInfo { character_id } => {
                Ok(format!("character:info:{}", character_id))
            }
            WorkerJob::UpdateAllianceInfo { alliance_id } => {
                Ok(format!("alliance:info:{}", alliance_id))
            }
            WorkerJob::UpdateCorporationInfo { corporation_id } => {
                Ok(format!("corporation:info:{}", corporation_id))
            }
            WorkerJob::UpdateAffiliations { character_ids } => {
                if character_ids.is_empty() {
                    return Err(WorkerError::InvalidJobIdentity(
                        "affiliation batch cannot be empty".to_string(),
                    )
                    .into());
                }

                if character_ids.len() > MAX_AFFILIATION_BATCH_SIZE {
                    return Err(WorkerError::AffiliationBatchTooLarge {
                        size: character_ids.len(),
                        max: MAX_AFFILIATION_BATCH_SIZE,
                    }
                    .into());
                }

                // Use hash of sorted IDs for uniqueness, plus count for retrieval from database
                // This keeps the identity string compact (always <50 bytes) regardless of batch size
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let mut sorted_ids = character_ids.clone();
                sorted_ids.sort_unstable();

                let mut hasher = DefaultHasher::new();
                sorted_ids.hash(&mut hasher);
                let hash = hasher.finish();

                // Format: "affiliation:batch:{count}:{hash}"
                // The count tells us how many IDs to retrieve from the database
                // The hash ensures uniqueness for different batches
                Ok(format!(
                    "affiliation:batch:{}:{:x}",
                    character_ids.len(),
                    hash
                ))
            }
        }
    }

    /// Parse identity string back into a WorkerJob
    ///
    /// Returns an error if the identity string is malformed or doesn't match any known job type.
    ///
    /// NOTE: For affiliation batches, the character IDs are NOT stored in the identity string.
    /// Only the count and hash are stored. The actual character IDs must be retrieved from
    /// the database when processing the job.
    ///
    /// # Examples
    ///
    /// ```
    /// // "character:info:123456" -> UpdateCharacterInfo { character_id: 123456 }
    /// // "alliance:info:789012" -> UpdateAllianceInfo { alliance_id: 789012 }
    /// // "corporation:info:345678" -> UpdateCorporationInfo { corporation_id: 345678 }
    /// // "affiliation:batch:100:a1b2c3d4e5f6g7h8" -> Returns error (cannot reconstruct IDs)
    /// ```
    pub fn parse_identity(identity: &str) -> Result<WorkerJob, Error> {
        let parts: Vec<&str> = identity.split(':').collect();

        match parts.as_slice() {
            ["character", "info", id_str] => {
                let character_id = id_str
                    .parse::<i64>()
                    .map_err(|_| WorkerError::InvalidJobIdentity(identity.to_string()))?;
                Ok(WorkerJob::UpdateCharacterInfo { character_id })
            }
            ["alliance", "info", id_str] => {
                let alliance_id = id_str
                    .parse::<i64>()
                    .map_err(|_| WorkerError::InvalidJobIdentity(identity.to_string()))?;
                Ok(WorkerJob::UpdateAllianceInfo { alliance_id })
            }
            ["corporation", "info", id_str] => {
                let corporation_id = id_str
                    .parse::<i64>()
                    .map_err(|_| WorkerError::InvalidJobIdentity(identity.to_string()))?;
                Ok(WorkerJob::UpdateCorporationInfo { corporation_id })
            }
            ["affiliation", "batch", count_str, _hash_str] => {
                // Affiliation batch identities only store count and hash, not actual IDs
                // The actual character IDs must be retrieved from the database
                // This parse_identity method cannot reconstruct the full job
                let _count = count_str
                    .parse::<usize>()
                    .map_err(|_| WorkerError::InvalidJobIdentity(identity.to_string()))?;

                // This will be fixed when WorkerJob is updated to use a count instead of Vec character IDs
                Err(WorkerError::InvalidJobIdentity(
                    "Cannot parse affiliation batch from identity - character IDs must be retrieved from database".to_string(),
                )
                .into())
            }
            _ => Err(WorkerError::InvalidJobIdentity(identity.to_string()).into()),
        }
    }
}
