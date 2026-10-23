//! Typed Docker CLI adapter. The CLI handles Engine transport and context credentials.
//! Compose membership never causes host files or a Compose configuration to be executed.
mod mutations;
mod parse;
mod process;
mod swarm;
use containerhub_core::*;
use parse::{invalid, json, text};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct DockerProvider {
    binary: PathBuf,
    context: String,
    environment: String,
    engine: Mutex<Option<String>>,
}
impl DockerProvider {
    pub fn new(context: &str, binary: Option<&Path>) -> Result<Self, ManagementError> {
        if context.is_empty()
            || context.len() > 128
            || !context.as_bytes()[0].is_ascii_alphanumeric()
            || !context
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "Docker context 名称无效",
            ));
        }
        let binary = match binary {
            Some(path) => {
                if !path.is_absolute() || !path.is_file() {
                    return Err(ManagementError::new(
                        ErrorKind::InvalidData,
                        "Docker CLI 路径必须是现有文件的绝对路径",
                    ));
                }
                path.to_owned()
            }
            None => [
                "/opt/homebrew/bin/docker",
                "/usr/local/bin/docker",
                "/usr/bin/docker",
            ]
            .into_iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
            .unwrap_or_else(|| "/usr/bin/docker".into()),
        };
        Ok(Self {
            binary,
            context: context.into(),
            environment: format!("docker:context:{context}"),
            engine: Mutex::new(None),
        })
    }
    pub fn environment(&self) -> &str {
        &self.environment
    }
    async fn run(&self, args: &[&str], write: bool) -> Result<Vec<u8>, ManagementError> {
        process::run(&self.binary, &self.context, args, write).await
    }
    async fn info(&self) -> Result<serde_json::Value, ManagementError> {
        let info = json(&self.run(&["info", "--format", "{{json .}}"], false).await?)?;
        let id = text(&info, "ID");
        if id.is_empty() {
            return Err(invalid());
        }
        let mut pinned = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        if pinned.as_ref().is_some_and(|pin| pin != id) {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "Docker context 已指向另一 Engine，请重新添加环境",
            ));
        }
        *pinned = Some(id.into());
        Ok(info)
    }
    async fn list(&self) -> Result<Vec<Container>, ManagementError> {
        let bytes = self
            .run(
                &["container", "ls", "--all", "--quiet", "--no-trunc"],
                false,
            )
            .await?;
        let ids = std::str::from_utf8(&bytes)
            .map_err(|_| invalid())?
            .lines()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        if ids
            .iter()
            .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(invalid());
        }
        let mut containers = vec![];
        for batch in ids.chunks(100) {
            let mut args = vec!["container", "inspect", "--"];
            args.extend(batch);
            let values = json(&self.run(&args, false).await?)?;
            for value in values.as_array().ok_or_else(invalid)? {
                containers.push(parse::container(value, &self.environment)?);
            }
        }
        Ok(containers)
    }
    async fn current(
        &self,
        key: &ResourceKey,
    ) -> Result<(Container, serde_json::Value), ManagementError> {
        if key.environment != self.environment
            || key.provider != ProviderKind::Docker
            || key.namespace.is_some()
            || key.incarnation.len() != 64
            || !key.incarnation.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "Docker 资源身份不匹配",
            ));
        }
        self.info().await?;
        let values = json(
            &self
                .run(&["container", "inspect", "--", &key.incarnation], false)
                .await?,
        )?;
        let value = values
            .as_array()
            .filter(|a| a.len() == 1)
            .and_then(|a| a.first())
            .ok_or_else(invalid)?
            .clone();
        let container = parse::container(&value, &self.environment)?;
        if container.key != *key {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "容器已被替换或重命名，请刷新",
            ));
        }
        Ok((container, value))
    }
    fn capabilities() -> Capabilities {
        Capabilities {
            stacks: true,
            inspect_container: true,
            create_container: true,
            create_and_start: true,
            delete_container: true,
            ..Default::default()
        }
    }
}
#[async_trait::async_trait]
impl ManagementClient for DockerProvider {
    async fn diagnostics(&self) -> Result<RuntimeDiagnostics, ManagementError> {
        let info = self.info().await;
        Ok(RuntimeDiagnostics {
            environment: self.environment.clone(),
            cli_path: Some(self.binary.display().to_string()),
            cli_version: info.as_ref().ok().map(|i| text(i, "ServerVersion").into()),
            observed_at: now(),
            service_error: info.err(),
            checks: RuntimeFeature::ALL
                .into_iter()
                .map(|feature| CapabilityCheck {
                    feature,
                    available: matches!(
                        feature,
                        RuntimeFeature::Stacks
                            | RuntimeFeature::Containers
                            | RuntimeFeature::Images
                            | RuntimeFeature::Volumes
                            | RuntimeFeature::Networks
                            | RuntimeFeature::InspectContainer
                            | RuntimeFeature::Lifecycle
                            | RuntimeFeature::CreateContainer
                            | RuntimeFeature::CreateAndStart
                            | RuntimeFeature::DeleteContainer
                    ),
                    reason: None,
                })
                .collect(),
        })
    }
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        let info = self.info().await?;
        Ok(Snapshot {
            swarm: self.swarm_inventory(&info).await,
            environment: self.environment.clone(),
            cli_path: self.binary.display().to_string(),
            cli_version: format!("Docker {}", text(&info, "ServerVersion")),
            observed_at: now(),
            containers: self.list().await?,
            lifecycle_supported: true,
            capabilities: Self::capabilities(),
            capability_note: None,
        })
    }
    async fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        self.info().await?;
        let entity = match kind {
            ResourceKind::Image => "image",
            ResourceKind::Volume => "volume",
            ResourceKind::Network => "network",
        };
        let bytes = self
            .run(&[entity, "ls", "--format", "{{json .}}"], false)
            .await?;
        std::str::from_utf8(&bytes)
            .map_err(|_| invalid())?
            .lines()
            .filter(|l| !l.is_empty())
            .map(|line| {
                let v = json(line.as_bytes())?;
                Ok(Resource {
                    id: if kind == ResourceKind::Volume {
                        text(&v, "Name")
                    } else {
                        text(&v, "ID")
                    }
                    .into(),
                    name: if kind == ResourceKind::Image {
                        format!("{}:{}", text(&v, "Repository"), text(&v, "Tag"))
                    } else {
                        text(&v, "Name").into()
                    },
                    kind,
                    detail: if kind == ResourceKind::Image {
                        text(&v, "Size")
                    } else {
                        text(&v, "Driver")
                    }
                    .into(),
                    size: None,
                })
            })
            .collect()
    }
    async fn inspect_container(
        &self,
        key: &ResourceKey,
    ) -> Result<ContainerDetails, ManagementError> {
        let (container, value) = self.current(key).await?;
        Ok(ContainerDetails {
            observed_at: now(),
            read_only: value["HostConfig"]["ReadonlyRootfs"].as_bool(),
            working_directory: None,
            user: None,
            environment_names: container.environment_names.clone(),
            label_names: container.label_names.clone(),
            argument_count: value["Config"]["Cmd"].as_array().map_or(0, Vec::len),
            container,
        })
    }
    async fn logs(&self, _: &ResourceKey, _: LogSource) -> Result<LogSnapshot, ManagementError> {
        Err(ManagementError::new(
            ErrorKind::Unsupported,
            "此 Docker 适配器尚未开放日志读取",
        ))
    }
    async fn lifecycle(
        &self,
        key: &ResourceKey,
        action: LifecycleAction,
    ) -> Result<(), ManagementError> {
        self.change_state(key, action).await
    }
    async fn execute(&self, command: &Command) -> Result<CommandResult, ManagementError> {
        self.apply(command, &ExecutionControl::default()).await
    }
    async fn execute_controlled(
        &self,
        command: &Command,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        self.apply(command, control).await
    }
}

#[cfg(test)]
mod tests;
