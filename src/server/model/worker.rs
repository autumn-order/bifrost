use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerJob {
    UpdateAllianceInfo { alliance_id: i64 },
}
