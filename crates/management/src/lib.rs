//! Shared management execution used by desktop, Agent and the Node bridge. No UI or transport dependency.
pub mod export;
pub mod queue;
pub mod recovery;
pub mod scheduler;
pub mod service;
pub mod storage;
use containerhub_core::*;

/// Preserve deduplication evidence until explicit maintenance; never silently evict old IDs.
pub const MAX_OPERATIONS: usize = 100_000;

pub enum OperationEvent {
    Ready,
    Operation(Box<Operation>),
    ControlError(String),
    StorageError,
    WriterUnavailable,
}

pub async fn execute(
    provider: &dyn ManagementClient,
    command: &Command,
) -> Result<CommandResult, ManagementError> {
    command.validate()?;
    provider.execute(command).await
}

#[cfg(test)]
mod cancellation_tests;
#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod workflow_tests;
