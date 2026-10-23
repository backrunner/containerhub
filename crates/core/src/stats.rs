use crate::ResourceKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerStats {
    pub key: ResourceKey,
    pub observed_at: u64,
    /// None means no valid interval sample, not zero CPU usage.
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
    pub memory_limit_bytes: Option<u64>,
}
