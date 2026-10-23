use super::*;

pub(super) async fn reconcile(
    id: u64,
    path: &Path,
    history: &mut [Operation],
    queue: &mut std::collections::VecDeque<Operation>,
) -> Result<Operation, ManagementError> {
    let mut op = history
        .iter()
        .find(|op| op.id == id)
        .cloned()
        .ok_or_else(|| ManagementError::new(ErrorKind::NotFound, "未找到该操作"))?;
    if op.state != OperationState::UnknownOutcome {
        return Ok(op);
    }
    if queue.len() >= 32 {
        return Err(ManagementError::new(
            ErrorKind::Conflict,
            "核对队列已满，请稍后重试",
        ));
    }
    let previous = op.clone();
    op.state = OperationState::Reconciling;
    op.finished_at = None;
    op.message = Some("正在核对原任务；不会重新执行命令".into());
    replace_operation(history, &op);
    if save_operations(path, history).await.is_err() {
        replace_operation(history, &previous);
        return Err(ManagementError::new(
            ErrorKind::Unavailable,
            "无法保存核对请求，未开始查询",
        ));
    }
    queue.push_back(op.clone());
    Ok(op)
}

pub(super) async fn cancel(
    id: u64,
    path: &Path,
    history: &mut [Operation],
    queue: &mut std::collections::VecDeque<Operation>,
    controls: &std::collections::HashMap<u64, ExecutionControl>,
) -> Result<Operation, ManagementError> {
    let mut op = history
        .iter()
        .find(|op| op.id == id)
        .cloned()
        .ok_or_else(|| ManagementError::new(ErrorKind::NotFound, "未找到该操作"))?;
    if !op.command.cancellable() {
        return Err(ManagementError::new(
            ErrorKind::Unsupported,
            "仅支持取消镜像拉取",
        ));
    }
    if !op.state.is_active() || op.cancel_requested {
        return Ok(op);
    }
    let previous = op.clone();
    op.cancel_requested = true;
    let queued = queue.iter().position(|q| q.id == id);
    let unstarted = queued.is_some_and(|index| queue[index].state == OperationState::Queued);
    if unstarted {
        op.state = OperationState::Cancelled;
        op.finished_at = Some(now());
        op.message = Some("已取消，尚未执行运行时命令".into());
    } else {
        op.message = Some("已请求取消，正在等待运行时核对结果".into());
    }
    replace_operation(history, &op);
    if save_operations(path, history).await.is_err() {
        replace_operation(history, &previous);
        return Err(ManagementError::new(
            ErrorKind::Unavailable,
            "无法保存取消请求，未发送取消",
        ));
    }
    if let Some(index) = queued {
        if unstarted {
            queue.remove(index);
        } else {
            queue[index] = op.clone();
        }
    }
    if let Some(control) = controls.get(&id) {
        control.cancel();
    }
    Ok(op)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancelling_a_queued_recovery_keeps_uncertainty_and_failed_save_never_enqueues() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("operations.json");
        let op = Operation {
            id: 1,
            command: Command::PullImage {
                environment: "remote".into(),
                reference: "example.invalid/test:v1".into(),
            },
            receipt: Some(ExecutionReceipt::Remote {
                agent: "a".into(),
                owner: "o".into(),
                request_id: "id".into(),
                expires_at: 1,
            }),
            state: OperationState::UnknownOutcome,
            result: None,
            progress: None,
            cancel_requested: false,
            started_at: 1,
            finished_at: Some(2),
            message: None,
        };
        let mut history = vec![op];
        let mut queue = std::collections::VecDeque::new();
        std::fs::create_dir(path.with_extension("json.tmp")).unwrap();
        assert!(reconcile(1, &path, &mut history, &mut queue).await.is_err());
        assert_eq!(history[0].state, OperationState::UnknownOutcome);
        assert!(queue.is_empty());
        std::fs::remove_dir(path.with_extension("json.tmp")).unwrap();
        reconcile(1, &path, &mut history, &mut queue).await.unwrap();
        let cancelled = cancel(1, &path, &mut history, &mut queue, &Default::default())
            .await
            .unwrap();
        assert_eq!(cancelled.state, OperationState::Reconciling);
        assert!(queue[0].cancel_requested);
        assert_eq!(
            queue.len(),
            1,
            "must query/cancel the original Agent request, not mark it unstarted"
        );
    }
}
