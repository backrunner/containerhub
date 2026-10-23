use crate::*;
use serde::{Deserialize, Serialize};

/// Public identifiers only. Persist before submitting a remote mutation; never a credential.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionReceipt {
    Remote {
        agent: String,
        owner: String,
        request_id: String,
        expires_at: u64,
    },
}

/// An authoritative terminal record; failure to obtain it is a separate, uncertain outcome.
#[derive(Clone, Debug)]
pub struct RecoveredExecution {
    pub state: OperationState,
    pub result: Option<CommandResult>,
    pub error: Option<ManagementError>,
    pub cancel_requested: bool,
}
