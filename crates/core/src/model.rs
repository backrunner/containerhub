use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    Apple,
    Docker,
    Kubernetes,
}

pub const LOCAL_ENVIRONMENT: &str = "apple:local:current-user";

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceKey {
    pub environment: String,
    #[serde(default)]
    pub provider: ProviderKind,
    #[serde(default)]
    pub namespace: Option<String>,
    pub id: String,
    /// Apple IDs are names. Creation time + digest distinguish replacements.
    pub incarnation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerState {
    Running,
    Stopped,
    Unknown(String),
}
impl ContainerState {
    pub fn label(&self) -> &str {
        match self {
            Self::Running => "运行中",
            Self::Stopped => "已停止",
            Self::Unknown(_) => "状态未知",
        }
    }
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub host: String,
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
}
impl Port {
    pub fn display(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!(
            "{host}:{} → {}/{}",
            self.host_port, self.container_port, self.protocol
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Container {
    pub key: ResourceKey,
    pub image: String,
    pub state: ContainerState,
    pub cpus: Option<u32>,
    pub memory_limit: Option<u64>,
    pub platform: String,
    pub ports: Vec<Port>,
    pub mounts: Vec<String>,
    pub networks: Vec<String>,
    pub addresses: Vec<String>,
    /// Values are discarded at the adapter boundary, never passed to UI/storage.
    pub environment_names: Vec<String>,
    /// Label values may contain credentials and never enter the inventory.
    #[serde(default)]
    pub label_names: Vec<String>,
    pub created_at: String,
    pub auto_remove: Option<bool>,
    #[serde(default)]
    pub health: crate::ContainerHealth,
    /// Only canonical Compose membership; arbitrary label values remain private.
    #[serde(default)]
    pub compose: Option<crate::ComposeMembership>,
}
impl Container {
    pub fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        self.key.id.to_lowercase().contains(&q)
            || self.image.to_lowercase().contains(&q)
            || self
                .label_names
                .iter()
                .any(|name| name.to_lowercase().contains(&q))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub swarm: crate::SwarmInventory,
    pub environment: String,
    pub cli_version: String,
    pub cli_path: String,
    pub observed_at: u64,
    pub containers: Vec<Container>,
    pub lifecycle_supported: bool,
    #[serde(default)]
    pub capabilities: crate::Capabilities,
    pub capability_note: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    Image,
    Volume,
    Network,
}
impl ResourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "镜像",
            Self::Volume => "存储卷",
            Self::Network => "网络",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub name: String,
    pub kind: ResourceKind,
    pub detail: String,
    pub size: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleAction {
    Start,
    Stop,
    Restart,
}
impl LifecycleAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "启动",
            Self::Stop => "停止",
            Self::Restart => "重启",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorKind {
    Cancelled,
    NotInstalled,
    Unavailable,
    Unsupported,
    Conflict,
    NotFound,
    Timeout,
    UnknownOutcome,
    InvalidData,
    PermissionDenied,
}
#[derive(Clone, Debug, thiserror::Error, Serialize, Deserialize)]
#[error("{message}")]
pub struct ManagementError {
    pub kind: ErrorKind,
    pub message: String,
}
impl ManagementError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{} MiB", bytes / (1024 * 1024))
    }
}
