use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerJob {
    UpdateAllianceInfo { alliance_id: i64 },
    UpdateCorporationInfo { corporation_id: i64 },
    UpdateCharacterInfo { character_id: i64 },
    UpdateAffiliations { count: u64 },
}
