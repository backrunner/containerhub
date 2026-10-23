use super::*;

impl AppleProvider {
    pub(super) async fn create_container(
        &self,
        spec: &CreateContainer,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        if spec.options.startup.is_some() {
            self.require(RuntimeFeature::StartupCommand).await?;
        }

        if self.list().await?.iter().any(|c| c.key.id == spec.name) {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "同名容器已存在，未创建或覆盖",
            ));
        }
        self.local_image(&spec.image).await?;
        for (kind, names) in [
            (ResourceKind::Network, spec.options.networks.clone()),
            (
                ResourceKind::Volume,
                spec.options
                    .volumes
                    .iter()
                    .map(|v| v.name.clone())
                    .collect(),
            ),
        ] {
            if !names.is_empty() {
                let available = self.resources(kind).await?;
                if names
                    .iter()
                    .any(|n| !available.iter().any(|r| &r.name == n))
                {
                    return Err(ManagementError::new(
                        ErrorKind::NotFound,
                        "所选网络或命名卷不存在，请刷新资源列表",
                    ));
                }
            }
        }
        let args = create_arguments(spec);
        let borrowed: Vec<_> = args.iter().map(String::as_str).collect();
        process::run_with_input(
            self.binary.as_ref().expect("version checked"),
            &borrowed,
            true,
            Duration::from_secs(300),
            control.secrets(),
        )
        .await?;
        let container = self
            .list()
            .await
            .map_err(|_| {
                ManagementError::new(
                    ErrorKind::UnknownOutcome,
                    "创建命令已返回，读取结果失败，请刷新核对",
                )
            })?
            .into_iter()
            .find(|c| c.key.id == spec.name && !c.created_at.is_empty())
            .ok_or_else(|| {
                ManagementError::new(
                    ErrorKind::UnknownOutcome,
                    "创建命令已返回，尚未找到目标容器，请刷新核对",
                )
            })?;
        Ok(CommandResult::Container(Box::new(container)))
    }
}
fn create_arguments(spec: &CreateContainer) -> Vec<String> {
    let mut args = vec![
        "create".into(),
        "--name".into(),
        spec.name.clone(),
        "--cpus".into(),
        spec.cpus.to_string(),
        "--memory".into(),
        format!("{}M", spec.memory_mib),
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
    for network in &spec.options.networks {
        args.extend(["--network".into(), network.clone()]);
    }
    for volume in &spec.options.volumes {
        args.extend([
            "--mount".into(),
            format!(
                "type=volume,source={},target={}{}",
                volume.name,
                volume.destination,
                if volume.read_only { ",readonly" } else { "" }
            ),
        ]);
    }
    if let Some(user) = &spec.options.user {
        args.extend(["--user".into(), user.clone()]);
    }
    if let Some(directory) = &spec.options.working_directory {
        args.extend(["--workdir".into(), directory.clone()]);
    }
    if spec.options.read_only {
        args.push("--read-only".into());
    }
    if !spec.options.secret_environment.is_empty() {
        args.extend(["--env-file".into(), "/dev/stdin".into()]);
    }
    if let Some(entrypoint) = spec
        .options
        .startup
        .as_ref()
        .and_then(|startup| startup.entrypoint.as_ref())
    {
        args.push(format!("--entrypoint={entrypoint}"));
    }
    args.extend([
        "--label".into(),
        "dev.containerhub.managed=true".into(),
        // The validated image is the first positional argument. Apple uses captureForPassthrough;
        // a terminator inside its remaining arguments would be a literal container argument.
        spec.image.clone(),
    ]);
    if let Some(startup) = &spec.options.startup {
        args.extend(startup.arguments.iter().cloned());
    }
    args
}
