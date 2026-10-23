use crate::*;
use serde::{Deserialize, Serialize};

/// Explicit allowlist: no environment values, process arguments or label values.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerDetails {
    pub container: Container,
    pub observed_at: u64,
    pub read_only: Option<bool>,
    pub working_directory: Option<String>,
    pub user: Option<String>,
    pub environment_names: Vec<String>,
    pub label_names: Vec<String>,
    pub argument_count: usize,
}
