use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateContainer {
    pub name: String,
    pub image: String,
    pub cpus: u32,
    pub memory_mib: u32,
    #[serde(default)]
    pub ports: Vec<Port>,
    #[serde(default)]
    pub options: CreateOptions,
}

/// Typed public operations. Host paths, credentials and arbitrary shell commands are not part of this contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    SwarmService {
        key: ResourceKey,
        stack: String,
        version: u64,
        action: SwarmAction,
    },
    StartRuntime {
        environment: String,
    },
    Lifecycle {
        key: ResourceKey,
        action: LifecycleAction,
    },
    PullImage {
        environment: String,
        reference: String,
    },
    CreateContainer {
        environment: String,
        spec: CreateContainer,
    },
    RebuildPorts {
        key: ResourceKey,
        expected_state: ContainerState,
        expected: Vec<Port>,
        ports: Vec<Port>,
    },
    DeleteContainer {
        key: ResourceKey,
    },
    DeleteImage {
        key: ImageKey,
    },
}
impl Command {
    pub fn starts_after_create(&self) -> bool {
        matches!(self, Self::CreateContainer { spec, .. } if spec.options.start_after_create)
    }
    pub fn cancellable(&self) -> bool {
        matches!(self, Self::PullImage { .. })
    }
    pub fn environment(&self) -> &str {
        match self {
            Self::StartRuntime { environment } => environment,
            Self::DeleteImage { key } => &key.environment,
            Self::SwarmService { key, .. }
            | Self::Lifecycle { key, .. }
            | Self::DeleteContainer { key }
            | Self::RebuildPorts { key, .. } => &key.environment,
            Self::PullImage { environment, .. } | Self::CreateContainer { environment, .. } => {
                environment
            }
        }
    }
    pub fn resource_key(&self) -> Option<&ResourceKey> {
        match self {
            Self::SwarmService { key, .. }
            | Self::Lifecycle { key, .. }
            | Self::DeleteContainer { key }
            | Self::RebuildPorts { key, .. } => Some(key),
            _ => None,
        }
    }
    pub fn target(&self) -> &str {
        match self {
            Self::StartRuntime { .. } => "运行时服务",
            Self::DeleteImage { key } => &key.reference,
            Self::SwarmService { key, .. }
            | Self::Lifecycle { key, .. }
            | Self::DeleteContainer { key }
            | Self::RebuildPorts { key, .. } => &key.id,
            Self::PullImage { reference, .. } => reference,
            Self::CreateContainer { spec, .. } => &spec.name,
        }
    }
    pub fn label(&self) -> &str {
        match self {
            Self::SwarmService { action, .. } => action.label(),
            Self::StartRuntime { .. } => "启动服务",
            Self::Lifecycle { action, .. } => action.label(),
            Self::PullImage { .. } => "拉取镜像",
            Self::CreateContainer { .. } if self.starts_after_create() => "创建并启动容器",
            Self::CreateContainer { .. } => "创建容器",
            Self::RebuildPorts { .. } => "重建端口映射",
            Self::DeleteContainer { .. } => "删除容器",
            Self::DeleteImage { .. } => "删除镜像",
        }
    }
    /// Lock by resource name, including replacements; immutable incarnation is still checked by the provider.
    pub fn lock_key(&self) -> String {
        let kind = if matches!(self, Self::SwarmService { .. }) {
            "swarm_service"
        } else if matches!(self, Self::StartRuntime { .. }) {
            "runtime"
        } else if matches!(self, Self::PullImage { .. } | Self::DeleteImage { .. }) {
            "image"
        } else {
            "container"
        };
        serde_json::to_string(&(self.environment(), kind, self.target())).expect("string tuple")
    }
    pub fn validate(&self) -> Result<(), ManagementError> {
        let invalid = || {
            ManagementError::new(
                ErrorKind::InvalidData,
                "操作参数无效，请检查名称、镜像、资源上限和端口",
            )
        };
        if self.environment().is_empty() || self.environment().len() > 256 {
            return Err(invalid());
        }
        match self {
            Self::StartRuntime { .. } => {}
            Self::DeleteImage { key } => key.validate()?,
            Self::SwarmService {
                key,
                stack,
                version,
                action,
            } => {
                if key.provider != ProviderKind::Docker
                    || key.namespace.as_deref() != Some("swarm-service")
                    || !valid_swarm_id(&key.incarnation)
                    || !valid_container_name(&key.id)
                    || !valid_container_name(stack)
                    || *version == 0
                    || matches!(action, SwarmAction::Scale{replicas} if *replicas>10000)
                {
                    return Err(invalid());
                }
            }
            Self::Lifecycle { key, .. }
            | Self::DeleteContainer { key }
            | Self::RebuildPorts { key, .. } => {
                if key.id.is_empty()
                    || key.id.len() > 256
                    || key.incarnation.is_empty()
                    || key.incarnation.len() > 1024
                {
                    return Err(invalid());
                }
            }
            Self::PullImage { reference, .. } => {
                if !valid_image_reference(reference) {
                    return Err(invalid());
                }
            }
            Self::CreateContainer { spec, .. } => {
                if !valid_container_name(&spec.name)
                    || !valid_image_reference(&spec.image)
                    || !(1..=64).contains(&spec.cpus)
                    || !(128..=262144).contains(&spec.memory_mib)
                    || spec.ports.len() > 32
                {
                    return Err(invalid());
                }
                spec.options.validate()?;
                let mut published = std::collections::HashSet::new();
                for port in &spec.ports {
                    if port.host.parse::<std::net::IpAddr>().is_err()
                        || port.host_port == 0
                        || port.container_port == 0
                        || !["tcp", "udp"].contains(&port.protocol.as_str())
                        || !published.insert((&port.host, port.host_port, &port.protocol))
                    {
                        return Err(invalid());
                    }
                }
            }
        }
        if let Self::RebuildPorts {
            expected,
            ports,
            expected_state,
            ..
        } = self
        {
            if matches!(expected_state, ContainerState::Unknown(_)) {
                return Err(invalid());
            }
            validate_ports(expected)?;
            validate_ports(ports)?;
        }
        Ok(())
    }
}
pub fn valid_container_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
pub fn valid_image_reference(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= 255
        && reference.as_bytes()[0].is_ascii_alphanumeric()
        && reference
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./-:@".contains(&b))
        && !reference.contains("://")
        && !reference.contains("..")
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CommandResult {
    Confirmed,
    Image(Box<Resource>),
    Container(Box<Container>),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub stacks: bool,
    #[serde(default)]
    pub inspect_container: bool,
    #[serde(default)]
    pub rebuild_ports: bool,
    #[serde(default)]
    pub cancel_pull: bool,
    #[serde(default)]
    pub inspect_image: bool,
    #[serde(default)]
    pub delete_image: bool,
    pub pull_image: bool,
    pub create_container: bool,
    #[serde(default)]
    pub create_advanced: bool,
    #[serde(default)]
    pub create_secrets: bool,
    #[serde(default)]
    pub create_startup: bool,
    #[serde(default)]
    pub create_and_start: bool,
    pub delete_container: bool,
    pub stats: bool,
}
impl Capabilities {
    pub fn apple_v1() -> Self {
        Self {
            stacks: false,
            inspect_container: true,
            rebuild_ports: true,
            cancel_pull: true,
            inspect_image: true,
            delete_image: true,
            pull_image: true,
            create_container: true,
            create_advanced: true,
            create_secrets: true,
            create_startup: true,
            create_and_start: true,
            delete_container: true,
            stats: true,
        }
    }
}
