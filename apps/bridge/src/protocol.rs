use containerhub_core::*;
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const MAX_FRAME: usize = 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub version: u32,
    pub id: String,
    pub request: Request,
}
#[derive(Deserialize)]
#[serde(
    tag = "method",
    content = "params",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Request {
    Hello,
    Snapshot,
    Stacks,
    StackCommands {
        #[serde(default)]
        kind: StackKind,
        environment: String,
        name: String,
        action: StackAction,
    },
    Diagnostics,
    HostSnapshot,
    Resources {
        kind: ResourceKind,
    },
    InspectImage {
        environment: String,
        reference: String,
    },
    ExportLogs {
        key: ResourceKey,
        source: LogSource,
        query: String,
    },
    Logs {
        key: ResourceKey,
        source: LogSource,
    },
    InspectContainer {
        key: ResourceKey,
    },
    Cancel {
        id: u64,
    },
    Reconcile {
        id: u64,
    },
    Stats {
        key: ResourceKey,
    },
    Submit {
        operation_id: u64,
        command: Command,
        #[serde(default)]
        secrets: SecretEnvironment,
    },
    Operation {
        id: u64,
    },
    Operations,
}
#[derive(Serialize)]
pub struct Response {
    pub version: u32,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ManagementError>,
}
impl Response {
    pub fn new(id: String, result: Result<serde_json::Value, ManagementError>) -> Self {
        match result {
            Ok(value) => Self {
                version: VERSION,
                id,
                result: Some(value),
                error: None,
            },
            Err(error) => Self {
                version: VERSION,
                id,
                result: None,
                error: Some(error),
            },
        }
    }
}
