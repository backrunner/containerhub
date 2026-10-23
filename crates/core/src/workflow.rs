//! Shared create/start orchestration. Runtime adapters only execute individual mutations.
use crate::*;

#[async_trait::async_trait]
pub trait CreationCheckpoint: Send + Sync {
    /// Must durably save the identity. An Agent also rechecks authorization here.
    async fn save(&self, container: &Container) -> Result<(), ManagementError>;
}

pub fn validate_created(command: &Command, container: &Container) -> Result<(), ManagementError> {
    let matches = match command {
        Command::CreateContainer { environment, spec } => {
            container.key.environment == *environment
                && container.key.id == spec.name
                && !container.key.incarnation.is_empty()
        }
        Command::RebuildPorts { key, ports, .. } => {
            container.key.environment == key.environment
                && container.key.provider == key.provider
                && container.key.namespace == key.namespace
                && container.key.id == key.id
                && container.key != *key
                && !container.key.incarnation.is_empty()
                && container.ports == *ports
        }
        _ => false,
    };
    if !matches {
        return Err(ManagementError::new(
            ErrorKind::UnknownOutcome,
            "创建结果身份不匹配，未继续启动",
        ));
    }
    Ok(())
}

/// Remote receipts delegate the whole workflow to the Agent; local execution persists between phases.
pub async fn execute_workflow(
    provider: &dyn ManagementClient,
    command: &Command,
    receipt: Option<&ExecutionReceipt>,
    control: &ExecutionControl,
) -> Result<CommandResult, ManagementError> {
    command.validate()?;
    control.secrets().validate_for(command)?;
    if !command.starts_after_create() || receipt.is_some() {
        return provider.execute_prepared(command, receipt, control).await;
    }
    if !control.has_checkpoint() {
        return Err(ManagementError::new(
            ErrorKind::Unsupported,
            "创建并启动需要持久化任务队列",
        ));
    }
    let snapshot = provider.snapshot().await?;
    if snapshot.environment != command.environment()
        || !snapshot.capabilities.create_container
        || !snapshot.capabilities.create_and_start
        || !snapshot.lifecycle_supported
    {
        return Err(ManagementError::new(
            ErrorKind::Unsupported,
            "当前环境不支持创建并启动容器",
        ));
    }
    let mut create = command.clone();
    if let Command::CreateContainer { spec, .. } = &mut create {
        spec.options.start_after_create = false;
    }
    control.report(OperationProgress::Creating);
    let CommandResult::Container(created) =
        provider.execute_prepared(&create, None, control).await?
    else {
        return Err(ManagementError::new(
            ErrorKind::UnknownOutcome,
            "创建结果缺少容器身份，未继续启动",
        ));
    };
    validate_created(command, &created)?;
    control
        .checkpoint_created(&created)
        .await
        .map_err(|error| {
            ManagementError::new(
                error.kind,
                format!("容器已创建，但未启动：{}", error.message),
            )
        })?;
    control.report(OperationProgress::Starting);
    provider
        .lifecycle(&created.key, LifecycleAction::Start)
        .await
        .map_err(|error| {
            ManagementError::new(
                error.kind,
                format!(
                    "容器已创建，启动未完成：{}；请核对该容器后重试启动",
                    error.message
                ),
            )
        })?;
    control.report(OperationProgress::Verifying);
    let observed = provider
        .snapshot()
        .await
        .ok()
        .and_then(|snapshot| {
            snapshot
                .containers
                .into_iter()
                .find(|c| c.key == created.key && c.state == ContainerState::Running)
        })
        .ok_or_else(|| {
            ManagementError::new(
                ErrorKind::UnknownOutcome,
                "容器已创建，但运行状态尚未确认；未重复创建或启动",
            )
        })?;
    Ok(CommandResult::Container(Box::new(observed)))
}

/// Retrying a partially completed create must never create again, even after external deletion.
pub fn start_created_command(
    command: &Command,
    result: Option<&CommandResult>,
    snapshot: &Snapshot,
) -> Option<Command> {
    if !(command.starts_after_create()
        || matches!(
            command,
            Command::RebuildPorts {
                expected_state: ContainerState::Running,
                ..
            }
        ))
        || !snapshot.lifecycle_supported
    {
        return None;
    }
    let CommandResult::Container(created) = result? else {
        return None;
    };
    validate_created(command, created).ok()?;
    snapshot.containers.iter().find(|c| {
        c.key == created.key
            && c.state == ContainerState::Stopped
            && validate_created(command, c).is_ok()
    })?;
    Some(Command::Lifecycle {
        key: created.key.clone(),
        action: LifecycleAction::Start,
    })
}
