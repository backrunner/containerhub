use super::*;

impl AppleProvider {
    pub(super) async fn lifecycle_command(
        &self,
        key: &ResourceKey,
        action: LifecycleAction,
    ) -> Result<(), ManagementError> {
        self.require(RuntimeFeature::Lifecycle).await?;
        let current = self.current(key).await?;
        if matches!(current.state, ContainerState::Unknown(_)) {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "当前容器状态不支持此操作",
            ));
        }
        if action == LifecycleAction::Restart && current.auto_remove == Some(true) {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "该容器会在停止后自动删除，不能重启",
            ));
        }
        if action == LifecycleAction::Start && current.state.is_running() {
            return Ok(());
        }
        if action == LifecycleAction::Stop && current.state == ContainerState::Stopped {
            return Ok(());
        }
        if action != LifecycleAction::Start && current.state.is_running() {
            self.run(&["stop", "--time", "10", "--", &key.id], true)
                .await?;
            self.confirm(key, false).await?;
        }
        if action != LifecycleAction::Stop {
            self.current(key).await?;
            self.run(&["start", "--", &key.id], true)
                .await
                .map_err(|e| {
                    if action == LifecycleAction::Restart {
                        ManagementError::new(
                            e.kind,
                            format!("停止已完成，但启动失败：{}", e.message),
                        )
                    } else {
                        e
                    }
                })?;
            self.confirm(key, true).await?;
        }
        Ok(())
    }
    pub(super) async fn local_image(&self, reference: &str) -> Result<Resource, ManagementError> {
        let bytes = self
            .run(&["image", "inspect", "--", reference], false)
            .await?;
        let mut images = parse_resources(&bytes, ResourceKind::Image)?;
        if images.len() != 1 {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "镜像结果不唯一或不可见",
            ));
        }
        Ok(images.remove(0))
    }
    pub(super) async fn apply(
        &self,
        command: &Command,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        command.validate()?;
        if command.starts_after_create() {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "请通过持久化任务队列创建并启动",
            ));
        }
        control.secrets().validate_for(command)?;
        if !control.secrets().is_empty() {
            self.require(RuntimeFeature::SecretEnvironment).await?;
        }
        // Prevent our own pull/create/delete-image calls from racing reference checks.
        // External CLI processes remain outside this provider-instance lock.
        let _image_guard = if matches!(
            command,
            Command::PullImage { .. }
                | Command::CreateContainer { .. }
                | Command::DeleteImage { .. }
                | Command::RebuildPorts { .. }
        ) {
            Some(tokio::select! {
                guard = self.image_writes.lock() => guard,
                _ = process::cancelled(control) => return Err(ManagementError::new(ErrorKind::Cancelled, "已取消，尚未执行运行时命令")),
            })
        } else {
            None
        };
        if command.environment() != LOCAL_ENVIRONMENT {
            return Err(ManagementError::new(ErrorKind::Conflict, "目标环境不匹配"));
        }
        self.require(match command {
            Command::SwarmService { .. } => {
                return Err(ManagementError::new(
                    ErrorKind::Unsupported,
                    "Apple container 不支持 Swarm",
                ));
            }
            Command::StartRuntime { .. } => RuntimeFeature::StartRuntime,
            Command::RebuildPorts { .. } => RuntimeFeature::RebuildPorts,
            Command::Lifecycle { .. } => RuntimeFeature::Lifecycle,
            Command::PullImage { .. } => RuntimeFeature::PullImage,
            Command::CreateContainer { spec, .. } if spec.options.has_advanced() => {
                RuntimeFeature::AdvancedCreate
            }
            Command::CreateContainer { .. } => RuntimeFeature::CreateContainer,
            Command::DeleteContainer { .. } => RuntimeFeature::DeleteContainer,
            Command::DeleteImage { .. } => RuntimeFeature::DeleteImage,
        })
        .await?;
        control.check_before_start()?;
        match command {
            Command::SwarmService { .. } => unreachable!("capability rejected"),
            Command::StartRuntime { .. } => self.start_runtime(control).await,
            Command::RebuildPorts {
                key,
                expected,
                ports,
                expected_state,
            } => {
                self.rebuild_ports(key, expected_state, expected, ports, control)
                    .await
            }
            Command::DeleteImage { key } => self.delete_image(key).await,
            Command::Lifecycle { key, action } => {
                self.lifecycle_command(key, *action).await?;
                Ok(CommandResult::Confirmed)
            }
            Command::PullImage { reference, .. } => {
                let path = self.binary.as_ref().expect("version checked");
                control.report(OperationProgress::Waiting);
                let pull = process::run_controlled(
                    path,
                    &["image", "pull", "--progress", "plain", "--", reference],
                    true,
                    Duration::from_secs(600),
                    Some(control),
                )
                .await;
                if let Err(error) = pull {
                    if control.is_cancelled() && error.kind == ErrorKind::UnknownOutcome {
                        control.report(OperationProgress::Verifying);
                        let observed = if self.local_image(reference).await.is_ok() {
                            "当前镜像引用可见"
                        } else {
                            "当前镜像引用未确认"
                        };
                        return Err(ManagementError::new(
                            ErrorKind::UnknownOutcome,
                            format!(
                                "已停止拉取客户端；{observed}。后台可能仍在处理，请稍后刷新核对；未回滚下载内容"
                            ),
                        ));
                    }
                    return Err(error);
                }
                control.report(OperationProgress::Verifying);
                self.local_image(reference)
                    .await
                    .map(|image| CommandResult::Image(Box::new(image)))
                    .map_err(|_| {
                        ManagementError::new(
                            ErrorKind::UnknownOutcome,
                            "拉取命令已完成，但镜像结果尚未确认，请刷新镜像列表",
                        )
                    })
            }
            Command::CreateContainer { spec, .. } => self.create_container(spec, control).await,
            Command::DeleteContainer { key } => {
                if self.current(key).await?.state != ContainerState::Stopped {
                    return Err(ManagementError::new(
                        ErrorKind::Conflict,
                        "请先停止容器；删除不会强制停止正在运行的资源",
                    ));
                }
                self.run(&["delete", "--", &key.id], true).await?;
                let remaining = self.list().await.map_err(|_| {
                    ManagementError::new(ErrorKind::UnknownOutcome, "删除命令已返回，无法核对结果")
                })?;
                if remaining.iter().any(|c| c.key.id == key.id) {
                    return Err(ManagementError::new(
                        ErrorKind::UnknownOutcome,
                        "同名资源仍存在或已被重建，请刷新核对，未继续删除",
                    ));
                }
                Ok(CommandResult::Confirmed)
            }
        }
    }
}
