//! Swarm service identities and task inventory. Task containers are not lifecycle targets.
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SwarmInventory {
    #[serde(default)]
    pub status: SwarmStatus,
    pub services: Vec<SwarmService>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmStatus {
    #[default]
    Inactive,
    Worker,
    Manager,
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmMode {
    Replicated,
    Global,
    Job,
    Unknown,
}
impl SwarmMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Replicated => "Replicated",
            Self::Global => "Global",
            Self::Job => "Job",
            Self::Unknown => "未知模式",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwarmService {
    pub key: ResourceKey,
    pub stack: String,
    pub version: u64,
    pub image: String,
    pub mode: SwarmMode,
    pub desired_replicas: Option<u64>,
    pub force_update: u64,
    pub update_state: Option<String>,
    pub ports: Vec<String>,
    pub tasks: Vec<SwarmTask>,
    pub tasks_truncated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwarmTask {
    pub id: String,
    pub node: String,
    pub slot: u64,
    pub desired_state: String,
    pub state: String,
    /// Actual container identity, when the scheduler has created one.
    /// Older agents may omit this; task IDs must never stand in for container IDs.
    #[serde(default)]
    pub container_id: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SwarmAction {
    Scale { replicas: u32 },
    Restart,
    Remove,
}
impl SwarmAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Scale { .. } => "更新副本数",
            Self::Restart => "滚动重启",
            Self::Remove => "移除服务",
        }
    }
}
impl SwarmService {
    pub fn running(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state == "running" && t.desired_state == "running")
            .count()
    }
    pub fn command(&self, action: SwarmAction) -> Result<Command, ManagementError> {
        if (matches!(action, SwarmAction::Scale { .. }) && self.mode != SwarmMode::Replicated)
            || (action == SwarmAction::Restart
                && !matches!(self.mode, SwarmMode::Replicated | SwarmMode::Global))
        {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "此服务模式不支持该操作",
            ));
        }
        let c = Command::SwarmService {
            key: self.key.clone(),
            stack: self.stack.clone(),
            version: self.version,
            action,
        };
        c.validate()?;
        Ok(c)
    }
}
pub fn valid_swarm_id(id: &str) -> bool {
    id.len() == 25
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}
