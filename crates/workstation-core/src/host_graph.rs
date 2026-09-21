use crate::{ownership::ProcessFact, Coverage};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutableObservation {
    pub process: crate::ownership::ProcessKey,
    pub path: String,
    pub disk_digest: Option<String>,
    pub matching_integrations: Vec<String>,
    pub loaded_image_identity_verified: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostGraph {
    pub observed_at: i64,
    pub uptime_ms: Option<u64>,
    pub coverage: Coverage,
    pub processes: Vec<ProcessFact>,
    pub executables: Vec<ExecutableObservation>,
    pub denied: u32,
    pub notices: Vec<String>,
}
