use super::*;

pub(super) struct Checkpoint {
    pub id: u64,
    pub container: Container,
    pub reply: tokio::sync::oneshot::Sender<Result<(), ManagementError>>,
}
pub(super) struct Journal {
    pub id: u64,
    pub tx: Sender<Checkpoint>,
}
#[async_trait::async_trait]
impl CreationCheckpoint for Journal {
    async fn save(&self, container: &Container) -> Result<(), ManagementError> {
        let (reply, receive) = tokio::sync::oneshot::channel();
        let unavailable = || {
            ManagementError::new(
                ErrorKind::Unavailable,
                "无法保存已创建容器的身份，未继续启动",
            )
        };
        self.tx
            .send(Checkpoint {
                id: self.id,
                container: container.clone(),
                reply,
            })
            .await
            .map_err(|_| unavailable())?;
        receive.await.map_err(|_| unavailable())?
    }
}

pub(super) async fn save(
    path: &Path,
    history: &mut [Operation],
    checkpoint: &Checkpoint,
) -> Result<Operation, ManagementError> {
    let op = history
        .iter_mut()
        .find(|op| op.id == checkpoint.id && op.state.is_active())
        .ok_or_else(|| ManagementError::new(ErrorKind::Conflict, "任务已不在执行中"))?;
    validate_created(&op.command, &checkpoint.container)?;
    if let Some(CommandResult::Container(old)) = &op.result
        && old.key != checkpoint.container.key
    {
        return Err(ManagementError::new(
            ErrorKind::UnknownOutcome,
            "原任务返回了不同的创建身份，已保留原记录",
        ));
    }
    op.result = Some(CommandResult::Container(Box::new(
        checkpoint.container.clone(),
    )));
    let op = op.clone();
    save_operations(path, history)
        .await
        .map_err(|_| ManagementError::new(ErrorKind::Unavailable, "创建结果无法写入任务记录"))?;
    Ok(op)
}
