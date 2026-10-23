use crate::*;
use std::sync::Arc;

/// An adapter's local identity is never exposed as another host's global identity.
pub struct EnvironmentClient {
    provider: Arc<dyn ManagementClient>,
    local: String,
    environment: String,
    kind: ProviderKind,
}
impl EnvironmentClient {
    pub fn new(
        provider: Arc<dyn ManagementClient>,
        local: String,
        environment: String,
        kind: ProviderKind,
    ) -> Self {
        Self {
            provider,
            local,
            environment,
            kind,
        }
    }
    fn local_key(&self, key: &ResourceKey) -> Result<ResourceKey, ManagementError> {
        if key.environment != self.environment || key.provider != self.kind {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "资源属于另一个环境或运行时",
            ));
        }
        let mut key = key.clone();
        key.environment = self.local.clone();
        Ok(key)
    }
}
#[async_trait::async_trait]
impl ManagementClient for EnvironmentClient {
    async fn host_snapshot(&self) -> Result<HostSnapshot, ManagementError> {
        let mut sample = self.provider.host_snapshot().await?;
        sample.validate()?;
        if sample.environment != self.local {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "主机采样环境不匹配",
            ));
        }
        sample.environment = self.environment.clone();
        Ok(sample)
    }
    async fn diagnostics(&self) -> Result<RuntimeDiagnostics, ManagementError> {
        let mut diagnostics = self.provider.diagnostics().await?;
        if diagnostics.environment != self.local {
            return Err(ManagementError::new(ErrorKind::Conflict, "诊断环境不匹配"));
        }
        diagnostics.environment = self.environment.clone();
        Ok(diagnostics)
    }
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        let mut snapshot = self.provider.snapshot().await?;
        if snapshot.environment != self.local
            || snapshot
                .containers
                .iter()
                .any(|c| c.key.environment != self.local || c.key.provider != self.kind)
        {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "运行时返回了不匹配的环境身份",
            ));
        }
        for service in &mut snapshot.swarm.services {
            if service.key.environment != self.local || service.key.provider != self.kind {
                return Err(ManagementError::new(
                    ErrorKind::InvalidData,
                    "Swarm 服务环境不匹配",
                ));
            }
            service.key.environment = self.environment.clone();
        }
        snapshot.environment = self.environment.clone();
        for container in &mut snapshot.containers {
            container.key.environment = self.environment.clone();
        }
        Ok(snapshot)
    }
    async fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        self.provider.resources(kind).await
    }
    async fn inspect_image(
        &self,
        environment: &str,
        reference: &str,
    ) -> Result<ImageDetails, ManagementError> {
        validate_image_query(environment, reference)?;
        if environment != self.environment {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "镜像属于另一个环境",
            ));
        }
        let mut image = self.provider.inspect_image(&self.local, reference).await?;
        if image.key.environment != self.local
            || image.key.provider != self.kind
            || image
                .used_by
                .iter()
                .any(|k| k.environment != self.local || k.provider != self.kind)
        {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "镜像详情环境不匹配",
            ));
        }
        image.key.environment = self.environment.clone();
        for key in &mut image.used_by {
            key.environment = self.environment.clone();
        }
        Ok(image)
    }
    async fn logs(
        &self,
        key: &ResourceKey,
        source: LogSource,
    ) -> Result<LogSnapshot, ManagementError> {
        let local = self.local_key(key)?;
        let mut logs = self.provider.logs(&local, source).await?;
        if logs.key != local {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "日志资源身份已改变",
            ));
        }
        logs.key = key.clone();
        Ok(logs)
    }
    async fn lifecycle(
        &self,
        key: &ResourceKey,
        action: LifecycleAction,
    ) -> Result<(), ManagementError> {
        self.provider.lifecycle(&self.local_key(key)?, action).await
    }
    async fn execute(&self, command: &Command) -> Result<CommandResult, ManagementError> {
        self.execute_controlled(command, &ExecutionControl::default())
            .await
    }
    async fn execute_controlled(
        &self,
        command: &Command,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        command.validate()?;
        if command.environment() != self.environment {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "目标属于另一个环境",
            ));
        }
        let local = match command {
            Command::SwarmService {
                key,
                stack,
                version,
                action,
            } => Command::SwarmService {
                key: self.local_key(key)?,
                stack: stack.clone(),
                version: *version,
                action: *action,
            },
            Command::StartRuntime { .. } => Command::StartRuntime {
                environment: self.local.clone(),
            },
            Command::DeleteImage { key } => {
                if key.provider != self.kind {
                    return Err(ManagementError::new(
                        ErrorKind::Conflict,
                        "镜像运行时不匹配",
                    ));
                }
                let mut key = key.clone();
                key.environment = self.local.clone();
                Command::DeleteImage { key }
            }
            Command::Lifecycle { key, action } => Command::Lifecycle {
                key: self.local_key(key)?,
                action: *action,
            },
            Command::RebuildPorts {
                key,
                expected,
                ports,
                expected_state,
            } => Command::RebuildPorts {
                key: self.local_key(key)?,
                expected: expected.clone(),
                expected_state: expected_state.clone(),
                ports: ports.clone(),
            },
            Command::DeleteContainer { key } => Command::DeleteContainer {
                key: self.local_key(key)?,
            },
            Command::PullImage { reference, .. } => Command::PullImage {
                environment: self.local.clone(),
                reference: reference.clone(),
            },
            Command::CreateContainer { spec, .. } => Command::CreateContainer {
                environment: self.local.clone(),
                spec: spec.clone(),
            },
        };
        let mut result = self.provider.execute_controlled(&local, control).await?;
        if let CommandResult::Container(container) = &mut result {
            if container.key.environment != self.local || container.key.provider != self.kind {
                return Err(ManagementError::new(
                    ErrorKind::InvalidData,
                    "创建结果环境不匹配",
                ));
            }
            container.key.environment = self.environment.clone();
        }
        Ok(result)
    }
    async fn inspect_container(
        &self,
        key: &ResourceKey,
    ) -> Result<ContainerDetails, ManagementError> {
        let local = self.local_key(key)?;
        let mut details = self.provider.inspect_container(&local).await?;
        if details.container.key != local {
            return Err(ManagementError::new(ErrorKind::Conflict, "容器身份已改变"));
        }
        details.container.key = key.clone();
        Ok(details)
    }
    async fn stats(&self, key: &ResourceKey) -> Result<ContainerStats, ManagementError> {
        let local = self.local_key(key)?;
        let mut stats = self.provider.stats(&local).await?;
        if stats.key != local {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "统计目标身份已改变",
            ));
        }
        stats.key = key.clone();
        Ok(stats)
    }
}
