use super::*;

impl RemoteClient {
    pub(super) fn prepare(&self, command: &Command) -> Result<ExecutionReceipt, ManagementError> {
        command.validate()?;
        if matches!(command, Command::StartRuntime { .. }) {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "服务启动目前仅限本机；远程环境请在目标机器启动服务",
            ));
        }
        Ok(ExecutionReceipt::Remote {
            agent: self.pin.id(),
            owner: self.credentials.owner_id().map_err(transport_error)?,
            request_id: nonce(),
            expires_at: now()
                + if matches!(
                    command,
                    Command::PullImage { .. }
                        | Command::CreateContainer { .. }
                        | Command::RebuildPorts { .. }
                ) {
                    900
                } else {
                    120
                },
        })
    }
    pub(super) fn original_request(
        &self,
        command: &Command,
        receipt: &ExecutionReceipt,
    ) -> Result<Mutation, ManagementError> {
        command.validate()?;
        let ExecutionReceipt::Remote {
            agent,
            owner,
            request_id,
            expires_at,
        } = receipt;
        if agent != &self.pin.id()
            || owner != &self.credentials.owner_id().map_err(transport_error)?
            || !(16..=128).contains(&request_id.len())
            || !request_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
            || *expires_at == 0
        {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "原任务的 Agent、所有者或请求关联不匹配；未发送命令",
            ));
        }
        Ok(Mutation {
            request_id: request_id.clone(),
            command: command.clone(),
            expires_at: *expires_at,
        })
    }
    fn validate_operation(
        &self,
        op: &RemoteOperation,
        request: &Mutation,
        receipt: &ExecutionReceipt,
    ) -> Result<(), ManagementError> {
        let ExecutionReceipt::Remote { agent, owner, .. } = receipt;
        if &op.agent != agent
            || &op.owner != owner
            || op.request.request_id != request.request_id
            || op.request.command != request.command
            || op.request.expires_at != request.expires_at
        {
            return Err(unexpected());
        }
        Ok(())
    }
    pub(super) async fn submit_prepared(
        &self,
        command: &Command,
        receipt: &ExecutionReceipt,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        control.check_before_start()?;
        control.secrets().validate_for(command)?;
        let request = self.original_request(command, receipt)?;
        if request.expires_at <= now() {
            return Err(ManagementError::new(
                ErrorKind::Timeout,
                "请求在发送前已过期，未执行命令",
            ));
        }
        let (mut secure, _) = authenticate(
            &self.endpoint,
            &self.credentials,
            Some(&self.pin),
            false,
            self.development,
        )
        .await
        .map_err(transport_error)?;
        control.check_before_start()?;
        let response = async {
            if control.secrets().is_empty() {
                secure.send(&Request::Submit(request.clone())).await?;
            } else {
                secure
                    .send(&Request::SubmitWithSecrets {
                        request: request.clone(),
                        secrets: control.secrets().clone(),
                    })
                    .await?;
            }
            secure.receive().await
        }
        .await;
        let initial = match response {
            Ok(Response::Operation(op)) => Some(op),
            Ok(Response::Error(error)) => return Err(error),
            // The request may have been accepted. Only query the persisted original ID.
            _ => None,
        };
        let remaining = Duration::from_secs(request.expires_at.saturating_sub(now()).max(1));
        let outcome = self
            .follow(&request, receipt, control, initial, remaining)
            .await
            .map_err(|error| uncertain(&request.request_id, &error.message))?;
        match outcome.state {
            OperationState::Succeeded => outcome
                .result
                .ok_or_else(|| uncertain(&request.request_id, "成功记录缺少结果")),
            OperationState::Cancelled => Err(ManagementError::new(
                ErrorKind::Cancelled,
                outcome
                    .error
                    .map(|e| e.message)
                    .unwrap_or("Agent 已确认取消".into()),
            )),
            OperationState::UnknownOutcome => Err(uncertain(
                &request.request_id,
                &outcome
                    .error
                    .map(|e| e.message)
                    .unwrap_or("Agent 结果待核对".into()),
            )),
            _ => Err(outcome.error.unwrap_or_else(unexpected)),
        }
    }
    /// Query-only continuation; an existing durable cancellation may also be delivered idempotently.
    pub(super) async fn follow(
        &self,
        request: &Mutation,
        receipt: &ExecutionReceipt,
        control: &ExecutionControl,
        mut operation: Option<RemoteOperation>,
        budget: Duration,
    ) -> Result<RecoveredExecution, ManagementError> {
        let deadline = tokio::time::Instant::now() + budget;
        let mut cancel_sent = false;
        let mut failures = 0;
        loop {
            if let Some(op) = operation.take() {
                self.validate_operation(&op, request, receipt)?;
                if request.command.starts_after_create()
                    && let Some(CommandResult::Container(created)) = &op.result
                {
                    containerhub_core::validate_created(&request.command, created)?;
                    control.checkpoint_created(created).await?;
                }
                if let Some(progress) = op.progress {
                    control.report(progress);
                }
                if !op.state.is_active() {
                    if op.state == OperationState::Succeeded && op.result.is_none() {
                        return Err(unexpected());
                    }
                    if op.state == OperationState::Failed && op.error.is_none() {
                        return Err(unexpected());
                    }
                    return Ok(RecoveredExecution {
                        state: op.state,
                        result: op.result,
                        error: op.error,
                        cancel_requested: op.cancel_requested,
                    });
                }
                // Do not use the original mutation expiry as a query deadline: accepted work may have finished later.
                if request.command.cancellable() && control.is_cancelled() && !cancel_sent {
                    match tokio::time::timeout_at(
                        deadline,
                        self.request(&Request::Cancel {
                            request_id: request.request_id.clone(),
                        }),
                    )
                    .await
                    {
                        Ok(Ok(Response::Operation(op))) => {
                            self.validate_operation(&op, request, receipt)?;
                            cancel_sent = op.cancel_requested || !op.state.is_active();
                            operation = Some(op);
                            continue;
                        }
                        Ok(Ok(Response::Error(error))) => return Err(error),
                        _ => return Err(uncertain(&request.request_id, "取消确认中断")),
                    }
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(uncertain(&request.request_id, "等待原任务超时"));
            }
            match tokio::time::timeout_at(
                deadline,
                self.request(&Request::Operation {
                    request_id: request.request_id.clone(),
                }),
            )
            .await
            {
                Ok(Ok(Response::Operation(op))) => {
                    operation = Some(op);
                    failures = 0;
                }
                Ok(Ok(Response::Error(error))) => return Err(error),
                Ok(Ok(_)) => return Err(unexpected()),
                _ => {
                    failures += 1;
                    if failures >= 3 {
                        return Err(uncertain(&request.request_id, "无法连接 Agent 查询原任务"));
                    }
                    tokio::time::sleep(Duration::from_millis(200 * failures)).await;
                }
            }
        }
    }
}
fn uncertain(id: &str, message: &str) -> ManagementError {
    ManagementError::new(
        ErrorKind::UnknownOutcome,
        format!("{message}；请求 ID：{id}；未重新提交"),
    )
}
