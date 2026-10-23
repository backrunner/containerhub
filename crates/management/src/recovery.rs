use containerhub_core::*;

/// Startup is local and bounded. Network recovery runs after Ready, through the normal scheduler.
pub async fn restore(
    provider: &dyn ManagementClient,
    history: &mut [Operation],
) -> std::collections::VecDeque<Operation> {
    let restored = if history.iter().any(|op| {
        op.receipt.is_none()
            && !matches!(op.command, Command::StartRuntime { .. })
            && matches!(
                op.state,
                OperationState::Running | OperationState::Reconciling
            )
    }) {
        provider.snapshot().await.ok()
    } else {
        None
    };
    let mut queue = std::collections::VecDeque::new();
    for op in history {
        if op.receipt.is_some()
            && matches!(
                op.state,
                OperationState::Running
                    | OperationState::Reconciling
                    | OperationState::UnknownOutcome
            )
        {
            if queue.len() < 32 {
                op.state = OperationState::Reconciling;
                op.finished_at = None;
                op.message = Some("正在查询 Agent 原任务；不会重复提交".into());
                queue.push_back(op.clone());
            } else {
                op.state = OperationState::UnknownOutcome;
                op.finished_at = Some(now());
                op.message = Some("自动核对已达 32 项，可在任务中心继续核对".into());
            }
            continue;
        }
        match op.state {
            OperationState::Submitting | OperationState::Queued => {
                op.state = OperationState::Failed;
                op.finished_at = Some(now());
                op.message = Some(
                    if op.command.secret_names().is_empty() {
                        "上次退出前尚未执行，可重新提交。"
                    } else {
                        "上次退出前尚未执行，秘密输入已丢失；请重新填写并创建新任务。"
                    }
                    .into(),
                );
            }
            OperationState::Running | OperationState::Reconciling => {
                if matches!(op.command, Command::StartRuntime { .. }) {
                    op.state = if runtime_started(provider, &op.command).await {
                        OperationState::Succeeded
                    } else {
                        OperationState::UnknownOutcome
                    };
                    op.finished_at = Some(now());
                    if op.state == OperationState::Succeeded {
                        op.result = Some(CommandResult::Confirmed);
                    }
                    op.message = Some(
                        if op.state == OperationState::Succeeded {
                            "已确认运行时服务可连接；未重复执行启动"
                        } else {
                            "运行时服务状态仍未确认；未重复执行启动"
                        }
                        .into(),
                    );
                    continue;
                }
                if created(op, restored.as_ref()) {
                    op.finished_at = Some(now());
                    continue;
                }
                let confirmed = confirmed(&op.command, restored.as_ref());
                op.state = if confirmed {
                    OperationState::Succeeded
                } else {
                    OperationState::UnknownOutcome
                };
                op.finished_at = Some(now());
                op.message = Some(
                    if confirmed {
                        "重新查询已确认目标状态；未重复执行命令"
                    } else {
                        "上次退出时操作未结束，结果待核对；未自动重试"
                    }
                    .into(),
                );
            }
            _ => {}
        }
    }
    queue
}

/// Host-service recovery requires diagnostics, even when inventory is unavailable.
pub async fn runtime_started(provider: &dyn ManagementClient, command: &Command) -> bool {
    matches!(command, Command::StartRuntime { .. })
        && provider.diagnostics().await.is_ok_and(|report| {
            report.environment == command.environment() && report.service_error.is_none()
        })
}

/// A durable create checkpoint proves which resource may be started, including after a crash.
pub fn created(op: &mut Operation, snapshot: Option<&Snapshot>) -> bool {
    if !op.command.starts_after_create() && !matches!(op.command, Command::RebuildPorts { .. }) {
        return false;
    }
    let Some(CommandResult::Container(container)) = &op.result else {
        return false;
    };
    if validate_created(&op.command, container).is_err() {
        return false;
    }
    let observed = snapshot.and_then(|s| s.containers.iter().find(|c| c.key == container.key));
    let desired = match &op.command {
        Command::RebuildPorts { expected_state, .. } => expected_state.clone(),
        _ => ContainerState::Running,
    };
    let (state, message) = match observed {
        Some(c) if validate_created(&op.command, c).is_err() => (
            OperationState::UnknownOutcome,
            "已创建容器的配置已改变，未确认完成；未重复执行",
        ),
        Some(c) if c.state == desired => (
            OperationState::Succeeded,
            "已确认创建结果及目标运行状态；未重复执行",
        ),
        Some(c) if c.state == ContainerState::Stopped && desired == ContainerState::Running => (
            OperationState::Failed,
            "容器已创建，目前处于停止状态；可单独启动该容器",
        ),
        _ => (
            OperationState::UnknownOutcome,
            "已保留创建结果，但目标运行状态未确认；未重复创建或启动",
        ),
    };
    if let Some(container) = observed {
        op.result = Some(CommandResult::Container(Box::new(container.clone())));
    }
    op.state = state;
    op.message = Some(message.into());
    true
}

/// Reconcile only observations that establish the requested result. Never replay a mutation.
pub fn confirmed(command: &Command, snapshot: Option<&Snapshot>) -> bool {
    let Some(snapshot) = snapshot else {
        return false;
    };
    match command {
        Command::Lifecycle { key, action } => snapshot
            .containers
            .iter()
            .find(|c| c.key == *key)
            .is_some_and(|c| match action {
                LifecycleAction::Start => c.state == ContainerState::Running,
                LifecycleAction::Stop => c.state == ContainerState::Stopped,
                LifecycleAction::Restart => false,
            }),
        // Absence alone cannot distinguish removal by this request from an external actor.
        _ => false,
    }
}
