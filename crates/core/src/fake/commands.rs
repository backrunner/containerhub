use super::*;

impl FakeProvider {
    pub(super) async fn apply(&self, command: &Command) -> Result<CommandResult, ManagementError> {
        command.validate()?;
        if command.starts_after_create() {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "请通过持久化任务队列创建并启动",
            ));
        }
        if command.environment() != "demo" {
            return Err(ManagementError::new(ErrorKind::Conflict, "示例环境不匹配"));
        }
        match command {
            Command::SwarmService { .. } => Err(ManagementError::new(
                ErrorKind::Unsupported,
                "示例运行时不执行 Swarm 操作",
            )),
            Command::StartRuntime { .. } => Ok(CommandResult::Confirmed),
            Command::RebuildPorts {
                key,
                expected,
                ports,
                expected_state,
            } => {
                let mut containers = self.containers.lock().unwrap();
                let index = containers
                    .iter()
                    .position(|c| c.key == *key)
                    .ok_or_else(|| {
                        ManagementError::new(
                            ErrorKind::Conflict,
                            "容器身份已改变，请重新打开端口编辑",
                        )
                    })?;
                if containers[index].ports != *expected
                    || containers[index].state != *expected_state
                {
                    return Err(ManagementError::new(
                        ErrorKind::Conflict,
                        "端口映射已被修改，请重新打开后核对",
                    ));
                }
                if containers.iter().any(|c| {
                    c.key != *key
                        && c.ports
                            .iter()
                            .any(|old| ports.iter().any(|new| ports_overlap(old, new)))
                }) {
                    return Err(ManagementError::new(
                        ErrorKind::Conflict,
                        "主机端口与其他容器冲突",
                    ));
                }
                if ports == expected {
                    return Ok(CommandResult::Container(Box::new(
                        containers[index].clone(),
                    )));
                }
                let old_key = containers[index].key.clone();
                containers[index].key.incarnation = format!(
                    "demo-rebuild-{}",
                    self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                );
                containers[index].created_at = now().to_string();
                {
                    let mut options = self.options.lock().unwrap();
                    if let Some(old) = options.remove(&old_key) {
                        options.insert(containers[index].key.clone(), old);
                    }
                }
                containers[index].ports = ports.clone();
                Ok(CommandResult::Container(Box::new(
                    containers[index].clone(),
                )))
            }
            Command::DeleteImage { key } => self.remove_image(key),
            Command::Lifecycle { key, action } => {
                self.lifecycle(key, *action).await?;
                Ok(CommandResult::Confirmed)
            }
            Command::PullImage { reference, .. } => {
                let image = Resource {
                    id: format!(
                        "{:064x}",
                        self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    ),
                    name: reference.clone(),
                    kind: ResourceKind::Image,
                    detail: "linux/arm64 · 示例".into(),
                    size: None,
                };
                let mut images = self.images.lock().unwrap();
                if let Some(old) = images.iter().find(|r| r.name == *reference) {
                    return Ok(CommandResult::Image(Box::new(old.clone())));
                }
                images.push(image.clone());
                Ok(CommandResult::Image(Box::new(image)))
            }
            Command::CreateContainer { spec, .. } => {
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
                    let rows = self.resources(kind).await?;
                    if names.iter().any(|n| !rows.iter().any(|r| &r.name == n)) {
                        return Err(ManagementError::new(
                            ErrorKind::NotFound,
                            "网络或命名卷不存在",
                        ));
                    }
                }
                let mut containers = self.containers.lock().unwrap();
                let images = self.images.lock().unwrap();
                if !images.iter().any(|r| r.name == spec.image) {
                    return Err(ManagementError::new(ErrorKind::NotFound, "请先拉取镜像"));
                }
                if containers.iter().any(|c| c.key.id == spec.name) {
                    return Err(ManagementError::new(ErrorKind::Conflict, "同名容器已存在"));
                }
                let incarnation = self
                    .next
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    .to_string();
                let container = Container {
                    key: ResourceKey {
                        environment: "demo".into(),
                        provider: ProviderKind::Apple,
                        namespace: None,
                        id: spec.name.clone(),
                        incarnation,
                    },
                    image: spec.image.clone(),
                    state: ContainerState::Stopped,
                    cpus: Some(spec.cpus),
                    memory_limit: Some(u64::from(spec.memory_mib) * 1024 * 1024),
                    platform: "linux / arm64".into(),
                    ports: spec.ports.clone(),
                    mounts: spec
                        .options
                        .volumes
                        .iter()
                        .map(|v| format!("{} → {}", v.name, v.destination))
                        .collect(),
                    networks: if spec.options.networks.is_empty() {
                        vec!["default".into()]
                    } else {
                        spec.options.networks.clone()
                    },
                    addresses: vec![],
                    environment_names: spec.options.secret_environment.clone(),
                    label_names: vec!["dev.containerhub.managed".into()],
                    created_at: now().to_string(),
                    auto_remove: Some(false),
                    compose: None,
                    health: crate::ContainerHealth::Unknown,
                };
                self.options
                    .lock()
                    .unwrap()
                    .insert(container.key.clone(), spec.options.clone());
                containers.push(container.clone());
                Ok(CommandResult::Container(Box::new(container)))
            }
            Command::DeleteContainer { key } => {
                let mut containers = self.containers.lock().unwrap();
                let index = containers
                    .iter()
                    .position(|c| c.key == *key)
                    .ok_or_else(|| ManagementError::new(ErrorKind::Conflict, "容器身份已改变"))?;
                if containers[index].state != ContainerState::Stopped {
                    return Err(ManagementError::new(
                        ErrorKind::Conflict,
                        "请先停止容器，再删除",
                    ));
                }
                containers.remove(index);
                self.options.lock().unwrap().remove(key);
                Ok(CommandResult::Confirmed)
            }
        }
    }
    pub(super) async fn sample(
        &self,
        key: &ResourceKey,
    ) -> Result<ContainerStats, ManagementError> {
        let containers = self.containers.lock().unwrap();
        let c = containers
            .iter()
            .find(|c| c.key == *key)
            .ok_or_else(|| ManagementError::new(ErrorKind::Conflict, "容器身份已改变"))?;
        if c.state != ContainerState::Running {
            return Err(ManagementError::new(ErrorKind::Unavailable, "容器未运行"));
        }
        Ok(ContainerStats {
            key: key.clone(),
            observed_at: now(),
            cpu_percent: Some(2.5),
            memory_bytes: Some(48 * 1024 * 1024),
            memory_limit_bytes: c.memory_limit,
        })
    }
}
