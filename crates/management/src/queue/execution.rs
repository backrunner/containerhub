use containerhub_core::*;
use std::sync::Arc;

pub(super) async fn run(
    provider: Arc<dyn ManagementClient>,
    mut op: Operation,
    recovering: bool,
    control: ExecutionControl,
) -> Operation {
    if recovering {
        if let Some(receipt) = &op.receipt {
            match provider
                .resume_execution(&op.command, receipt, &control)
                .await
            {
                Ok(outcome) if !outcome.state.is_active() => {
                    op.state = outcome.state;
                    op.result = outcome.result.or(op.result);
                    op.cancel_requested |= outcome.cancel_requested;
                    op.message = Some(
                        outcome
                            .error
                            .map(|e| e.message)
                            .unwrap_or("已查询 Agent 原任务记录；未重复执行命令".into()),
                    );
                }
                Ok(_) => uncertain(&mut op, "Agent 没有返回终态，稍后可再次核对"),
                Err(error) => uncertain(
                    &mut op,
                    &format!("无法核对原任务：{}；未重新提交", error.message),
                ),
            }
        } else if matches!(op.command, Command::StartRuntime { .. }) {
            if crate::recovery::runtime_started(provider.as_ref(), &op.command).await {
                op.state = OperationState::Succeeded;
                op.result = Some(CommandResult::Confirmed);
                op.message = Some("已确认运行时服务可连接；未重复执行启动".into());
            } else {
                uncertain(&mut op, "服务仍不可连接；未重复执行启动");
            }
        } else {
            let snapshot = provider.snapshot().await.ok();
            if crate::recovery::created(&mut op, snapshot.as_ref()) {
                // Only observe the saved incarnation. Never replay either phase.
            } else if crate::recovery::confirmed(&op.command, snapshot.as_ref()) {
                op.state = OperationState::Succeeded;
                op.result = Some(CommandResult::Confirmed);
                op.message = Some("已重新查询同一资源的目标状态；未重复执行命令".into());
            } else {
                uncertain(&mut op, "没有足够证据确认原操作结果；未自动重试");
            }
        }
    } else {
        match execute_workflow(
            provider.as_ref(),
            &op.command,
            op.receipt.as_ref(),
            &control,
        )
        .await
        {
            Ok(result) => {
                op.result = Some(result);
                op.state = OperationState::Succeeded;
                op.message = Some("已重新查询并确认操作结果".into());
            }
            Err(error) => {
                if let Some(created) = control.created() {
                    op.result = Some(CommandResult::Container(Box::new(created)));
                }
                op.state = match error.kind {
                    ErrorKind::Cancelled => OperationState::Cancelled,
                    ErrorKind::UnknownOutcome => OperationState::UnknownOutcome,
                    _ => OperationState::Failed,
                };
                op.message = Some(error.message);
            }
        }
    }
    if op.result.is_none() {
        op.result = control
            .created()
            .map(|c| CommandResult::Container(Box::new(c)));
    }
    op.progress = control.progress().or(op.progress);
    op.cancel_requested |= control.is_cancelled();
    op.finished_at = Some(now());
    op
}
fn uncertain(op: &mut Operation, message: &str) {
    op.state = OperationState::UnknownOutcome;
    op.message = Some(message.into());
}
