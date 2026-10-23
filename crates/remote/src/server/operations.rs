use super::*;
use containerhub_core::ExecutionControl;

impl Agent {
    pub(super) async fn submit(
        self: &Arc<Self>,
        auth: &Authorization,
        request: Mutation,
        secrets: SecretEnvironment,
    ) -> Result<Response> {
        ensure!(
            request.command.environment() == self.environment,
            "wrong environment"
        );
        authorize_command(&self.store, auth, &request.command)?;
        request.command.validate()?;
        let json = serde_json::to_string(&request)?;
        if let Some((original, result)) = self
            .store
            .operation(&request.request_id, &auth.owner_id())?
        {
            ensure!(
                original == json,
                "request ID reused with a different mutation"
            );
            if !secrets.is_empty() {
                return Ok(Response::Error(ManagementError::new(
                    ErrorKind::Conflict,
                    "秘密输入只能提交一次；请查询原任务 ID",
                )));
            }
            return Ok(Response::Operation(serde_json::from_str(&result)?));
        }
        if let Err(error) = secrets.validate_for(&request.command) {
            return Ok(Response::Error(error));
        }
        ensure!(
            request.expires_at > now() && request.expires_at.saturating_sub(now()) <= 900,
            "mutation expired or deadline too far away"
        );
        let initial = RemoteOperation {
            request: request.clone(),
            owner: auth.owner_id(),
            device: auth.device_id(),
            agent: self.public_key().id(),
            accepted_at: now(),
            state: OperationState::Queued,
            error: None,
            result: None,
            progress: None,
            cancel_requested: false,
        };
        let reservation = match self.scheduler.reserve(&request.command) {
            Ok(slot) => slot,
            Err(error) => return Ok(Response::Error(error)),
        };
        let has_secrets = !secrets.is_empty();
        let control = ExecutionControl::with_checkpoint(
            secrets,
            Arc::new(checkpoint::Journal {
                store: self.store.clone(),
                auth: Authorization {
                    grant: auth.grant().clone(),
                },
                operation: initial.clone(),
            }),
        );
        let (created, result) = {
            let mut controls = self.controls.lock().expect("controls");
            let accepted = self.store.accept_operation(
                &request.request_id,
                &auth.owner_id(),
                &json,
                &serde_json::to_string(&initial)?,
            )?;
            if accepted.0 {
                controls.insert(request.request_id.clone(), control.clone());
            }
            accepted
        };
        if !created && has_secrets {
            return Ok(Response::Error(ManagementError::new(
                ErrorKind::Conflict,
                "秘密输入只能提交一次；请查询原任务 ID",
            )));
        }
        if created {
            let agent = self.clone();
            let device = Authorization {
                grant: auth.grant().clone(),
            };
            tokio::spawn(async move {
                let _permit = tokio::select! {
                    permit = reservation.enter() => Some(permit),
                    _ = cancelled(&control) => None,
                };
                let mut operation = initial;
                let gate = authorize_command(&agent.store, &device, &request.command);
                if control.is_cancelled() {
                    operation.state = OperationState::Cancelled;
                    operation.error = Some(ManagementError::new(
                        ErrorKind::Cancelled,
                        "已取消，尚未执行运行时命令",
                    ));
                } else if let Err(error) = gate {
                    operation.state = OperationState::Failed;
                    operation.error = Some(ManagementError::new(
                        ErrorKind::PermissionDenied,
                        error.to_string(),
                    ));
                } else if request.expires_at <= now() {
                    operation.state = OperationState::Failed;
                    operation.error =
                        Some(ManagementError::new(ErrorKind::Timeout, "操作执行前已过期"));
                } else {
                    let started = {
                        let _controls = agent.controls.lock().expect("controls");
                        // Serialize this transition with cancel: a durable queued cancellation never executes.
                        if control.is_cancelled() {
                            false
                        } else {
                            operation.state = OperationState::Running;
                            agent
                                .store
                                .finish_operation(
                                    &request.request_id,
                                    &serde_json::to_string(&operation).expect("operation"),
                                )
                                .is_ok()
                        }
                    };
                    if !started {
                        if control.is_cancelled() {
                            operation.state = OperationState::Cancelled;
                            operation.error = Some(ManagementError::new(
                                ErrorKind::Cancelled,
                                "已取消，尚未执行运行时命令",
                            ));
                        } else {
                            operation.state = OperationState::Failed;
                            operation.error = Some(ManagementError::new(
                                ErrorKind::Unavailable,
                                "无法保存执行记录，未执行命令",
                            ));
                        }
                    } else {
                        let deadline = if matches!(
                            request.command,
                            Command::PullImage { .. }
                                | Command::CreateContainer { .. }
                                | Command::RebuildPorts { .. }
                        ) {
                            660
                        } else {
                            120
                        };
                        let outcome = tokio::time::timeout(
                            Duration::from_secs(deadline),
                            containerhub_core::execute_workflow(
                                agent.provider.as_ref(),
                                &request.command,
                                None,
                                &control,
                            ),
                        )
                        .await
                        .unwrap_or_else(|_| {
                            Err(ManagementError::new(
                                ErrorKind::UnknownOutcome,
                                "Agent 等待运行时超时；请核对实际资源状态",
                            ))
                        });
                        match outcome {
                            Ok(result) => {
                                operation.result = Some(result);
                                operation.state = OperationState::Succeeded;
                            }
                            Err(error) => {
                                operation.result = control.created().map(|container| {
                                    containerhub_core::CommandResult::Container(Box::new(container))
                                });
                                operation.state = match error.kind {
                                    ErrorKind::Cancelled => OperationState::Cancelled,
                                    ErrorKind::UnknownOutcome => OperationState::UnknownOutcome,
                                    _ => OperationState::Failed,
                                };
                                operation.error = Some(error);
                            }
                        }
                    }
                }
                let mut controls = agent.controls.lock().expect("controls");
                operation.progress = control.progress();
                operation.cancel_requested = control.is_cancelled();
                let _ = agent.store.finish_operation(
                    &request.request_id,
                    &serde_json::to_string(&operation).expect("operation"),
                );
                controls.remove(&request.request_id);
            });
        }
        Ok(Response::Operation(serde_json::from_str(&result)?))
    }
    pub(super) fn cancel(&self, auth: &Authorization, id: &str) -> Result<Response> {
        let controls = self.controls.lock().expect("controls");
        let (_, json) = self
            .store
            .operation(id, &auth.owner_id())?
            .context("operation not found")?;
        let mut operation: RemoteOperation = serde_json::from_str(&json)?;
        self.store.authorize(
            auth,
            permission(&operation.request.command)?,
            operation.request.command.environment(),
            now(),
        )?;
        ensure!(
            operation.request.command.cancellable(),
            "only image pulls support cancellation"
        );
        if operation.state.is_active() && !operation.cancel_requested {
            let control = controls
                .get(id)
                .context("operation execution unavailable; reconcile before retrying")?;
            operation.cancel_requested = true;
            operation.progress = control.progress();
            if operation.state == OperationState::Queued {
                operation.state = OperationState::Cancelled;
                operation.error = Some(ManagementError::new(
                    ErrorKind::Cancelled,
                    "已取消，尚未执行运行时命令",
                ));
            }
            self.store
                .finish_operation(id, &serde_json::to_string(&operation)?)?;
            control.cancel();
        }
        Ok(Response::Operation(operation))
    }
}
async fn cancelled(control: &ExecutionControl) {
    while !control.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
