//! UI-independent resource identity, snapshots and management contracts.

pub mod stacks;
pub use stacks::*;
pub mod host;
pub use host::*;
pub mod ports;
pub use ports::*;
mod commands;
pub mod discovery;
pub mod inventory;
pub use discovery::*;
pub use inventory::*;
pub mod create_options;
pub mod execution;
pub mod inspect;
pub use create_options::*;
pub use execution::*;
pub use inspect::*;
pub mod environment;
pub mod fake;
pub mod images;
pub mod log_export;
pub mod logs;
pub mod model;
pub mod operation;
pub mod receipt;
pub use receipt::*;
pub mod secrets;
pub mod startup;
pub mod stats;
pub mod workflow;
pub use secrets::*;
pub use startup::*;
pub use workflow::*;

use async_trait::async_trait;
pub use commands::*;
pub use images::*;
pub use log_export::LogExport;
pub use logs::{LogSnapshot, LogSource};
pub use model::*;
pub use operation::*;
pub use stats::*;

#[async_trait]
pub trait ManagementClient: Send + Sync {
    /// No runtime mutation or network submission. The caller must persist the returned receipt.
    fn prepare_execution(
        &self,
        _command: &Command,
    ) -> Result<Option<ExecutionReceipt>, ManagementError> {
        Ok(None)
    }
    async fn execute_prepared(
        &self,
        command: &Command,
        receipt: Option<&ExecutionReceipt>,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        if receipt.is_some() {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "任务关联不属于当前运行时",
            ));
        }
        self.execute_controlled(command, control).await
    }
    /// Follow the original request only. Never resubmit a mutation during recovery.
    async fn resume_execution(
        &self,
        _command: &Command,
        _receipt: &ExecutionReceipt,
        _control: &ExecutionControl,
    ) -> Result<RecoveredExecution, ManagementError> {
        Err(ManagementError::new(
            ErrorKind::Unsupported,
            "当前运行时无法查询原任务关联",
        ))
    }
    async fn host_snapshot(&self) -> Result<HostSnapshot, ManagementError> {
        Err(ManagementError::new(
            ErrorKind::Unsupported,
            "此运行时未提供主机采样",
        ))
    }
    async fn diagnostics(&self) -> Result<RuntimeDiagnostics, ManagementError> {
        Err(ManagementError::new(
            ErrorKind::Unsupported,
            "此运行时尚未提供能力诊断",
        ))
    }
    async fn snapshot(&self) -> Result<Snapshot, ManagementError>;
    async fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, ManagementError>;
    async fn inspect_image(
        &self,
        _environment: &str,
        _reference: &str,
    ) -> Result<ImageDetails, ManagementError> {
        Err(ManagementError::new(
            ErrorKind::Unsupported,
            "此运行时尚未支持镜像详情",
        ))
    }
    async fn logs(
        &self,
        key: &ResourceKey,
        source: LogSource,
    ) -> Result<LogSnapshot, ManagementError>;
    /// A write must revalidate the resource incarnation before issuing a command.
    async fn lifecycle(
        &self,
        key: &ResourceKey,
        action: LifecycleAction,
    ) -> Result<(), ManagementError>;
    async fn execute(&self, command: &Command) -> Result<CommandResult, ManagementError> {
        command.validate()?;
        match command {
            Command::Lifecycle { key, action } => {
                self.lifecycle(key, *action).await?;
                Ok(CommandResult::Confirmed)
            }
            _ => Err(ManagementError::new(
                ErrorKind::Unsupported,
                "此运行时尚未支持该操作",
            )),
        }
    }
    async fn execute_controlled(
        &self,
        command: &Command,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        control.check_before_start()?;
        control.secrets().validate_for(command)?;
        if !control.secrets().is_empty() {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "此运行时尚未支持秘密注入",
            ));
        }
        self.execute(command).await
    }
    async fn inspect_container(
        &self,
        _key: &ResourceKey,
    ) -> Result<ContainerDetails, ManagementError> {
        Err(ManagementError::new(
            ErrorKind::Unsupported,
            "此运行时尚未支持容器检查",
        ))
    }
    async fn stats(&self, _key: &ResourceKey) -> Result<ContainerStats, ManagementError> {
        Err(ManagementError::new(
            ErrorKind::Unsupported,
            "此运行时尚未支持实时统计",
        ))
    }
}

mod swarm;
pub use swarm::*;
