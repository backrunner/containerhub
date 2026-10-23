use super::*;
impl DockerProvider {
    pub(super) async fn change_state(
        &self,
        key: &ResourceKey,
        action: LifecycleAction,
    ) -> Result<(), ManagementError> {
        let (c, raw) = self.current(key).await?;
        if raw["Config"]["Labels"]["com.docker.swarm.service.id"].is_string() {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "请在 Stack 中管理 Swarm 服务",
            ));
        }
        if matches!(c.state, ContainerState::Unknown(_))
            || (action == LifecycleAction::Restart && c.auto_remove == Some(true))
        {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "该容器当前不支持此操作",
            ));
        }
        if (action == LifecycleAction::Start && c.state.is_running())
            || (action == LifecycleAction::Stop && c.state == ContainerState::Stopped)
        {
            return Ok(());
        }
        let verb = match action {
            LifecycleAction::Start => "start",
            LifecycleAction::Stop => "stop",
            LifecycleAction::Restart => "restart",
        };
        self.run(&["container", verb, "--", &key.incarnation], true)
            .await?;
        let expected = if action == LifecycleAction::Stop {
            ContainerState::Stopped
        } else {
            ContainerState::Running
        };
        for _ in 0..10 {
            if let Ok((c, _)) = self.current(key).await
                && c.state == expected
            {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        Err(ManagementError::new(
            ErrorKind::UnknownOutcome,
            "Docker 已接受操作，目标状态尚未确认",
        ))
    }
    pub(super) async fn apply(
        &self,
        command: &Command,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        command.validate()?;
        control.check_before_start()?;
        control.secrets().validate_for(command)?;
        if command.environment() != self.environment {
            return Err(ManagementError::new(ErrorKind::Conflict, "目标环境不匹配"));
        }
        if !control.secrets().is_empty() || command.starts_after_create() {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "请通过受支持的创建表单与任务队列执行",
            ));
        }
        match command {
            Command::SwarmService {
                key,
                stack,
                version,
                action,
            } => self.change_service(key, stack, *version, *action).await,
            Command::Lifecycle { key, action } => {
                self.change_state(key, *action).await?;
                Ok(CommandResult::Confirmed)
            }
            Command::DeleteContainer { key } => {
                let (current, raw) = self.current(key).await?;
                if raw["Config"]["Labels"]["com.docker.swarm.service.id"].is_string() {
                    return Err(ManagementError::new(
                        ErrorKind::Unsupported,
                        "请在 Stack 中管理 Swarm 服务",
                    ));
                }
                if current.state != ContainerState::Stopped {
                    return Err(ManagementError::new(ErrorKind::Conflict, "请先停止容器"));
                }
                self.run(&["container", "rm", "--", &key.incarnation], true)
                    .await?;
                if self
                    .list()
                    .await
                    .map_err(|_| {
                        ManagementError::new(ErrorKind::UnknownOutcome, "删除结果尚未确认")
                    })?
                    .iter()
                    .any(|c| c.key.id == key.id)
                {
                    return Err(ManagementError::new(
                        ErrorKind::UnknownOutcome,
                        "同名容器仍存在，请刷新核对",
                    ));
                }
                Ok(CommandResult::Confirmed)
            }
            Command::CreateContainer { spec, .. } => {
                if spec.options.has_advanced()
                    || spec.options.startup.is_some()
                    || !spec.options.secret_environment.is_empty()
                {
                    return Err(ManagementError::new(
                        ErrorKind::Unsupported,
                        "Docker 首版仅支持基础容器创建",
                    ));
                }
                self.info().await?;
                let mut args = vec![
                    "container".to_string(),
                    "create".into(),
                    "--pull=never".into(),
                    "--name".into(),
                    spec.name.clone(),
                    "--cpus".into(),
                    spec.cpus.to_string(),
                    "--memory".into(),
                    format!("{}m", spec.memory_mib),
                    "--label".into(),
                    "dev.containerhub.managed=true".into(),
                ];
                for port in &spec.ports {
                    let host = if port.host.contains(':') {
                        format!("[{}]", port.host)
                    } else {
                        port.host.clone()
                    };
                    args.extend([
                        "--publish".into(),
                        format!(
                            "{host}:{}:{}/{}",
                            port.host_port, port.container_port, port.protocol
                        ),
                    ]);
                }
                args.extend(["--".into(), spec.image.clone()]);
                let out = self
                    .run(&args.iter().map(String::as_str).collect::<Vec<_>>(), true)
                    .await?;
                let id = String::from_utf8_lossy(&out).trim().to_string();
                let key = ResourceKey {
                    environment: self.environment.clone(),
                    provider: ProviderKind::Docker,
                    namespace: None,
                    id: spec.name.clone(),
                    incarnation: id,
                };
                let (created, _) = self.current(&key).await.map_err(|_| {
                    ManagementError::new(ErrorKind::UnknownOutcome, "容器可能已创建，身份尚未确认")
                })?;
                Ok(CommandResult::Container(Box::new(created)))
            }
            _ => Err(ManagementError::new(
                ErrorKind::Unsupported,
                "此 Docker 适配器尚未支持该操作",
            )),
        }
    }
}
