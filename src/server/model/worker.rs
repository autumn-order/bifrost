use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerJob {
    UpdateAlliance { alliance_id: i32 },
}
