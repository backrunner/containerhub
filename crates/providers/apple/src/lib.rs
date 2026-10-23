//! Public Apple CLI only; no shell, private database or XPC access.
#[cfg(all(test, unix))]
mod contract_tests;
mod create;
mod discovery;
mod images;
pub use discovery::validate_binary;
mod inspect;
mod mutations;
mod parse;
mod process;
mod rebuild;
mod stats;
mod system;

use async_trait::async_trait;
use containerhub_core::*;
pub use parse::{parse_containers, parse_resources};
use std::{path::PathBuf, time::Duration};

pub struct AppleProvider {
    host: std::sync::Arc<containerhub_host::HostSampler>,
    discovery: tokio::sync::Mutex<Option<discovery::Cached>>,
    image_writes: tokio::sync::Mutex<()>,
    runtime_starts: tokio::sync::Mutex<()>,
    binary: Option<PathBuf>,
    samples: std::sync::Mutex<stats::Samples>,
}
impl Default for AppleProvider {
    fn default() -> Self {
        Self {
            host: Default::default(),
            discovery: Default::default(),
            image_writes: Default::default(),
            runtime_starts: Default::default(),
            samples: Default::default(),
            binary: ["/opt/homebrew/bin/container", "/usr/local/bin/container"]
                .into_iter()
                .map(PathBuf::from)
                .find(|p| p.is_file()),
        }
    }
}
impl AppleProvider {
    async fn run(&self, args: &[&str], write: bool) -> Result<Vec<u8>, ManagementError> {
        let path = self.binary.as_ref().ok_or_else(|| {
            ManagementError::new(
                ErrorKind::NotInstalled,
                "未找到 Apple container。请先安装官方运行时，然后重新打开应用。",
            )
        })?;
        process::run(path, args, write).await
    }
    async fn list(&self) -> Result<Vec<Container>, ManagementError> {
        self.require(RuntimeFeature::Containers).await?;
        parse_containers(
            &self
                .run(&["list", "--all", "--format", "json"], false)
                .await?,
        )
    }
    async fn version(&self) -> Result<String, ManagementError> {
        let path = self.binary.as_ref().ok_or_else(|| {
            ManagementError::new(ErrorKind::NotInstalled, "未找到 Apple container")
        })?;
        let bytes =
            process::run_with_timeout(path, &["--version"], false, Duration::from_secs(2)).await?;
        let text = String::from_utf8_lossy(&bytes);
        text.strip_prefix("container CLI version ")
            .and_then(|s| s.split_whitespace().next())
            .map(str::to_string)
            .ok_or_else(|| {
                ManagementError::new(ErrorKind::Unsupported, "无法识别 Apple container 版本")
            })
    }
    async fn current(&self, key: &ResourceKey) -> Result<Container, ManagementError> {
        if key.environment != LOCAL_ENVIRONMENT
            || key.provider != ProviderKind::Apple
            || key.namespace.is_some()
            || !valid_id(&key.id)
        {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "目标环境或容器标识无效",
            ));
        }
        let current = self
            .list()
            .await?
            .into_iter()
            .find(|c| c.key.id == key.id)
            .ok_or_else(|| ManagementError::new(ErrorKind::NotFound, "容器已不存在，请刷新列表"))?;
        if current.key != *key || current.created_at.is_empty() {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "同名容器已被替换或身份不完整，请重新选择",
            ));
        }
        Ok(current)
    }
    async fn confirm(&self, key: &ResourceKey, running: bool) -> Result<(), ManagementError> {
        for _ in 0..12 {
            let c = self.current(key).await.map_err(|e| {
                if e.kind == ErrorKind::NotFound {
                    ManagementError::new(
                        ErrorKind::UnknownOutcome,
                        "操作后容器已被运行时移除；可能启用了 --rm，未尝试重建",
                    )
                } else {
                    e
                }
            })?;
            if (running && c.state == ContainerState::Running)
                || (!running && c.state == ContainerState::Stopped)
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Err(ManagementError::new(
            ErrorKind::UnknownOutcome,
            "命令已结束，但尚未观察到目标状态；请刷新核对，勿重复提交",
        ))
    }
}
fn valid_id(id: &str) -> bool {
    valid_container_name(id)
}
#[async_trait]
impl ManagementClient for AppleProvider {
    async fn host_snapshot(&self) -> Result<HostSnapshot, ManagementError> {
        let host = self.host.clone();
        tokio::task::spawn_blocking(move || host.sample(LOCAL_ENVIRONMENT))
            .await
            .map_err(|_| ManagementError::new(ErrorKind::Unavailable, "主机采样任务中断"))?
    }
    async fn diagnostics(&self) -> Result<RuntimeDiagnostics, ManagementError> {
        let mut report = self.discover(true).await;
        if report.service_error.is_none() {
            report.service_error = self.service_status().await.err();
        }
        Ok(report)
    }
    async fn inspect_image(
        &self,
        environment: &str,
        reference: &str,
    ) -> Result<ImageDetails, ManagementError> {
        self.require(RuntimeFeature::InspectImage).await?;
        self.image_details(environment, reference).await
    }
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        let report = self.discover(false).await;
        if let Some(error) = report.service_error {
            return Err(error);
        }
        self.service_status().await?;
        let supported = report.supports(RuntimeFeature::Lifecycle);
        Ok(Snapshot {
            swarm: Default::default(),
            environment: LOCAL_ENVIRONMENT.into(),
            cli_version: report.cli_version.clone().unwrap_or_default(),
            cli_path: self.binary.as_ref().unwrap().display().to_string(),
            observed_at: now(),
            containers: self.list().await?,
            lifecycle_supported: supported,
            capabilities: report.capabilities(),
            capability_note: (!supported)
                .then(|| "部分操作不可用，详情见设置中的运行时能力检查".into()),
        })
    }
    async fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        self.require(match kind {
            ResourceKind::Image => RuntimeFeature::Images,
            ResourceKind::Volume => RuntimeFeature::Volumes,
            ResourceKind::Network => RuntimeFeature::Networks,
        })
        .await?;
        let command = match kind {
            ResourceKind::Image => "image",
            ResourceKind::Volume => "volume",
            ResourceKind::Network => "network",
        };
        parse_resources(
            &self
                .run(&[command, "list", "--format", "json"], false)
                .await?,
            kind,
        )
    }
    async fn logs(
        &self,
        key: &ResourceKey,
        source: LogSource,
    ) -> Result<LogSnapshot, ManagementError> {
        self.require(if source == LogSource::Boot {
            RuntimeFeature::BootLogs
        } else {
            RuntimeFeature::Logs
        })
        .await?;
        self.current(key).await?;
        let args = match source {
            LogSource::Standard => vec!["logs", "-n", "500", "--", &key.id],
            LogSource::Boot => vec!["logs", "--boot", "-n", "500", "--", &key.id],
        };
        let bytes = self.run(&args, false).await?;
        // Detect deletion/recreation while the CLI was reading.
        self.current(key).await?;
        Ok(LogSnapshot::from_bytes(key.clone(), source, &bytes))
    }
    async fn lifecycle(
        &self,
        key: &ResourceKey,
        action: LifecycleAction,
    ) -> Result<(), ManagementError> {
        self.lifecycle_command(key, action).await
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
    async fn inspect_container(
        &self,
        key: &ResourceKey,
    ) -> Result<ContainerDetails, ManagementError> {
        self.require(RuntimeFeature::InspectContainer).await?;
        self.container_details(key).await
    }
    async fn stats(&self, key: &ResourceKey) -> Result<ContainerStats, ManagementError> {
        self.require(RuntimeFeature::Stats).await?;
        self.read_stats(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_ids_cannot_be_flags_or_shell_fragments() {
        for s in ["--all", "x;rm", "../x", "", "$(echo x)", "a b", "-a"] {
            assert!(!valid_id(s));
        }
        assert!(valid_id("containerhub-smoke_1"));
    }
}
