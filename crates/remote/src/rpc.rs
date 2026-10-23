use containerhub_core::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "StoredMutation")]
pub struct Mutation {
    pub request_id: String,
    pub command: Command,
    pub expires_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteOperation {
    pub request: Mutation,
    pub owner: String,
    pub device: String,
    pub agent: String,
    pub accepted_at: u64,
    pub state: OperationState,
    pub error: Option<ManagementError>,
    #[serde(default)]
    pub result: Option<CommandResult>,
    #[serde(default)]
    pub progress: Option<OperationProgress>,
    #[serde(default)]
    pub cancel_requested: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "method",
    content = "params",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Request {
    Snapshot,
    Diagnostics,
    HostSnapshot,
    Resources(ResourceKind),
    InspectImage {
        environment: String,
        reference: String,
    },
    Logs {
        key: ResourceKey,
        source: LogSource,
    },
    #[serde(alias = "lifecycle")]
    Submit(Mutation),
    SubmitWithSecrets {
        request: Mutation,
        secrets: SecretEnvironment,
    },
    InspectContainer {
        key: ResourceKey,
    },
    Cancel {
        request_id: String,
    },
    Stats {
        key: ResourceKey,
    },
    Operation {
        request_id: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Response {
    Snapshot(Snapshot),
    Diagnostics(RuntimeDiagnostics),
    HostSnapshot(HostSnapshot),
    Stats(ContainerStats),
    Resources(Vec<Resource>),
    ImageDetails(ImageDetails),
    ContainerDetails(ContainerDetails),
    Logs(LogSnapshot),
    Operation(RemoteOperation),
    Error(ManagementError),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredMutation {
    request_id: String,
    command: Option<Command>,
    key: Option<ResourceKey>,
    action: Option<LifecycleAction>,
    expires_at: u64,
}
impl TryFrom<StoredMutation> for Mutation {
    type Error = &'static str;
    fn try_from(w: StoredMutation) -> Result<Self, Self::Error> {
        let command = match (w.command, w.key, w.action) {
            (Some(c), None, None) => c,
            (None, Some(key), Some(action)) => Command::Lifecycle { key, action },
            _ => return Err("ambiguous mutation"),
        };
        Ok(Self {
            request_id: w.request_id,
            command,
            expires_at: w.expires_at,
        })
    }
}
