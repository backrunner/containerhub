use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationState {
    Submitting,
    Queued,
    Running,
    Reconciling,
    Succeeded,
    Failed,
    Cancelled,
    UnknownOutcome,
}
impl OperationState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Submitting => "提交中",
            Self::Queued => "排队中",
            Self::Running => "执行中",
            Self::Reconciling => "核对状态",
            Self::Succeeded => "已完成",
            Self::Cancelled => "已取消",
            Self::Failed => "失败",
            Self::UnknownOutcome => "结果待核对",
        }
    }
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Submitting | Self::Queued | Self::Running | Self::Reconciling
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "StoredOperation")]
pub struct Operation {
    pub id: u64,
    pub command: Command,
    #[serde(default)]
    pub receipt: Option<ExecutionReceipt>,
    #[serde(default)]
    pub result: Option<CommandResult>,
    #[serde(default)]
    pub progress: Option<OperationProgress>,
    #[serde(default)]
    pub cancel_requested: bool,
    pub state: OperationState,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub message: Option<String>,
}
impl Operation {
    pub fn status_label(&self) -> &'static str {
        if self.command.starts_after_create()
            && matches!(self.result, Some(CommandResult::Container(_)))
        {
            match self.state {
                OperationState::Failed => return "已创建 · 启动未完成",
                OperationState::UnknownOutcome => return "已创建 · 启动待核对",
                _ => {}
            }
        }
        self.state.label()
    }
}

#[derive(Deserialize)]
struct StoredOperation {
    id: u64,
    receipt: Option<ExecutionReceipt>,
    command: Option<Command>,
    key: Option<ResourceKey>,
    action: Option<LifecycleAction>,
    state: OperationState,
    started_at: u64,
    finished_at: Option<u64>,
    message: Option<String>,
    result: Option<CommandResult>,
    progress: Option<OperationProgress>,
    #[serde(default)]
    cancel_requested: bool,
}
impl TryFrom<StoredOperation> for Operation {
    type Error = &'static str;
    fn try_from(w: StoredOperation) -> Result<Self, Self::Error> {
        let command = match (w.command, w.key, w.action) {
            (Some(command), None, None) => command,
            (None, Some(key), Some(action)) => Command::Lifecycle { key, action },
            _ => return Err("ambiguous or missing operation command"),
        };
        Ok(Self {
            id: w.id,
            command,
            receipt: w.receipt,
            result: w.result,
            progress: w.progress,
            cancel_requested: w.cancel_requested,
            state: w.state,
            started_at: w.started_at,
            finished_at: w.finished_at,
            message: w.message,
        })
    }
}
