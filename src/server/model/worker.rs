use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkerJob {
    UpdateAllianceInfo { alliance_id: i64 },
    UpdateCorporationInfo { corporation_id: i64 },
    UpdateCharacterInfo { character_id: i64 },
    UpdateAffiliations { character_ids: Vec<i64> },
}

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
