//! Explicit demo provider. Never chosen as a fallback for a failed live connection.
mod commands;
mod images;
use crate::*;
use async_trait::async_trait;
use std::sync::Mutex;

pub struct FakeProvider {
    containers: Mutex<Vec<Container>>,
    images: Mutex<Vec<Resource>>,
    options: Mutex<std::collections::HashMap<ResourceKey, CreateOptions>>,
    next: std::sync::atomic::AtomicU64,
}
impl Default for FakeProvider {
    fn default() -> Self {
        let names = [
            "web-api",
            "postgres",
            "redis",
            "minio",
            "nginx-proxy",
            "worker",
            "sandbox",
            "mailpit",
        ];
        let images = [
            "ghcr.io/example/api:dev",
            "postgres:17-alpine",
            "redis:7-alpine",
            "minio/minio:latest",
            "nginx:alpine",
            "ghcr.io/example/api:dev",
            "alpine:3.22",
            "axllent/mailpit:latest",
        ];
        Self {
            images: Mutex::new(
                images
                    .iter()
                    .enumerate()
                    .filter(|(i, name)| !images[..*i].contains(name))
                    .map(|(i, name)| Resource {
                        id: format!("{i:064x}"),
                        name: name.to_string(),
                        kind: ResourceKind::Image,
                        detail: "linux/arm64 · 示例".into(),
                        size: None,
                    })
                    .collect(),
            ),
            options: Default::default(),
            next: std::sync::atomic::AtomicU64::new(10),
            containers: Mutex::new(
                names
                    .iter()
                    .enumerate()
                    .map(|(i, name)| Container {
                        key: ResourceKey {
                            provider: crate::ProviderKind::Apple,
                            namespace: None,
                            environment: "demo".into(),
                            id: name.to_string(),
                            incarnation: format!("demo-{i}"),
                        },
                        image: images[i].into(),
                        state: if i < 5 {
                            ContainerState::Running
                        } else {
                            ContainerState::Stopped
                        },
                        cpus: Some(2),
                        memory_limit: Some(512 * 1024 * 1024),
                        platform: "linux / arm64".into(),
                        ports: if i == 0 {
                            vec![Port {
                                host: "127.0.0.1".into(),
                                host_port: 3000,
                                container_port: 3000,
                                protocol: "tcp".into(),
                            }]
                        } else {
                            vec![]
                        },
                        mounts: if i == 1 {
                            vec!["database-data → /var/lib/postgresql/data".into()]
                        } else {
                            vec![]
                        },
                        networks: vec!["default".into()],
                        addresses: vec![],
                        environment_names: vec!["PATH".into()],
                        label_names: vec!["dev.containerhub.demo".into()],
                        created_at: "2026-10-22T00:00:00Z".into(),
                        auto_remove: Some(false),
                        compose: None,
                        health: match i {
                            0..=2 => ContainerHealth::Healthy,
                            3 => ContainerHealth::Unhealthy,
                            _ => ContainerHealth::Unconfigured,
                        },
                    })
                    .collect(),
            ),
        }
    }
}
#[async_trait]
impl ManagementClient for FakeProvider {
    async fn host_snapshot(&self) -> Result<HostSnapshot, ManagementError> {
        let wave = (now() as f64 * 0.11).sin();
        let gib = 1024 * 1024 * 1024;
        Ok(HostSnapshot {
            environment: "demo".into(),
            observed_at: now(),
            hostname: Some("demo.local".into()),
            os: Some("macOS 26".into()),
            architecture: "arm64".into(),
            processor: Some("Apple M4 Pro".into()),
            logical_cpus: 12,
            uptime_seconds: 12 * 86400 + 18 * 3600,
            cpu_percent: Some(24.6 + wave * 5.),
            memory_used_bytes: Some(12 * gib),
            memory_total_bytes: Some(32 * gib),
            disk_path: Some("/".into()),
            disk_used_bytes: Some(142 * gib),
            disk_total_bytes: Some(512 * gib),
            disk_read_bytes_per_second: Some((2. + wave) * 1e6),
            disk_write_bytes_per_second: Some(1.2e6),
            network_receive_bytes_per_second: Some((1.24 + wave * 0.2) * 1e6),
            network_send_bytes_per_second: Some(0.32e6),
            sample: true,
        })
    }
    async fn diagnostics(&self) -> Result<RuntimeDiagnostics, ManagementError> {
        Ok(RuntimeDiagnostics {
            environment: "demo".into(),
            cli_path: None,
            cli_version: Some("示例".into()),
            observed_at: now(),
            service_error: None,
            checks: RuntimeFeature::ALL
                .into_iter()
                .map(|feature| CapabilityCheck {
                    feature,
                    available: feature != RuntimeFeature::Stacks,
                    reason: None,
                })
                .collect(),
        })
    }
    async fn inspect_image(
        &self,
        environment: &str,
        reference: &str,
    ) -> Result<ImageDetails, ManagementError> {
        self.describe_image(environment, reference)
    }
    async fn inspect_container(
        &self,
        key: &ResourceKey,
    ) -> Result<ContainerDetails, ManagementError> {
        let containers = self.containers.lock().unwrap();
        let container = containers
            .iter()
            .find(|c| c.key == *key)
            .cloned()
            .ok_or_else(|| ManagementError::new(ErrorKind::Conflict, "容器身份已改变"))?;
        let options = self
            .options
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .unwrap_or_default();
        Ok(ContainerDetails {
            environment_names: container.environment_names.clone(),
            container,
            observed_at: now(),
            read_only: Some(options.read_only),
            working_directory: options.working_directory,
            user: options.user,
            label_names: vec!["dev.containerhub.managed".into()],
            argument_count: options.startup.as_ref().map_or(0, |s| s.arguments.len()),
        })
    }
    async fn execute_controlled(
        &self,
        command: &Command,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        control.check_before_start()?;
        control.secrets().validate_for(command)?;
        let result = self.apply(command).await;
        if command.cancellable() && result.is_ok() {
            control.report(OperationProgress::Verifying);
        }
        result
    }
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        Ok(Snapshot {
            swarm: Default::default(),
            environment: "demo".into(),
            cli_version: "示例".into(),
            cli_path: "不连接运行时".into(),
            observed_at: now(),
            containers: self.containers.lock().unwrap().clone(),
            lifecycle_supported: true,
            capabilities: Capabilities {
                rebuild_ports: true,
                ..Capabilities::apple_v1()
            },
            capability_note: None,
        })
    }
    async fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        Ok(match kind {
            ResourceKind::Image => self.images.lock().unwrap().clone(),
            ResourceKind::Volume => vec![Resource {
                id: "database-data".into(),
                name: "database-data".into(),
                kind,
                detail: "local · ext4".into(),
                size: Some(512 * 1024 * 1024 * 1024),
            }],
            ResourceKind::Network => vec![Resource {
                id: "default".into(),
                name: "default".into(),
                kind,
                detail: "nat · 192.168.64.0/24".into(),
                size: None,
            }],
        })
    }
    async fn logs(
        &self,
        key: &ResourceKey,
        source: LogSource,
    ) -> Result<LogSnapshot, ManagementError> {
        if !self
            .containers
            .lock()
            .unwrap()
            .iter()
            .any(|c| &c.key == key)
        {
            return Err(ManagementError::new(ErrorKind::NotFound, "示例容器不存在"));
        }
        let text = match source {
            LogSource::Standard => format!("[示例日志] {}\n10:24:01 INFO  service initialized\n10:24:02 INFO  listening on 0.0.0.0:3000\n10:24:08 INFO  GET /health 200  2ms\n10:24:12 INFO  已完成数据同步\n10:24:16 WARN  retrying upstream connection\n10:24:17 INFO  upstream connected\n", key.id),
            LogSource::Boot => "[示例启动日志]\n[0.000000] Linux version 6.18.15\n[0.162305] virtio_net: network device ready\n[0.283100] init: starting container process\n".into(),
        };
        Ok(LogSnapshot::from_bytes(
            key.clone(),
            source,
            text.as_bytes(),
        ))
    }
    async fn lifecycle(
        &self,
        key: &ResourceKey,
        action: LifecycleAction,
    ) -> Result<(), ManagementError> {
        let mut containers = self.containers.lock().unwrap();
        let c = containers
            .iter_mut()
            .find(|c| c.key == *key)
            .ok_or_else(|| ManagementError::new(ErrorKind::NotFound, "容器已不存在或已被替换"))?;
        c.state = if action == LifecycleAction::Stop {
            ContainerState::Stopped
        } else {
            ContainerState::Running
        };
        Ok(())
    }
    async fn execute(&self, command: &Command) -> Result<CommandResult, ManagementError> {
        SecretEnvironment::default().validate_for(command)?;
        self.apply(command).await
    }
    async fn stats(&self, key: &ResourceKey) -> Result<ContainerStats, ManagementError> {
        self.sample(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_matches_name_and_image_case_insensitively() {
        let p = FakeProvider::default();
        let c = &p.containers.lock().unwrap()[0];
        assert!(c.matches(" WEB-API "));
        assert!(c.matches("GHCR.IO"));
        assert!(c.matches(" CONTAINERHUB.DEMO "));
        assert!(!c.matches("postgres"));
    }
    #[test]
    fn identity_distinguishes_recreated_containers() {
        let a = ResourceKey {
            provider: crate::ProviderKind::Apple,
            namespace: None,
            environment: "local".into(),
            id: "db".into(),
            incarnation: "first".into(),
        };
        let mut b = a.clone();
        b.incarnation = "replacement".into();
        assert_ne!(a, b);
        b = a.clone();
        b.environment = "remote".into();
        assert_ne!(a, b);
    }
}
