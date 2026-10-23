//! Compose and Swarm inventory and actions shared by native, Agent and Node clients.
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
mod overview;
mod status;
pub use overview::StackContainerOverview;
pub use status::StackServiceStatus;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComposeMembership {
    pub project: String,
    pub service: String,
    #[serde(default)]
    pub one_off: bool,
}
impl ComposeMembership {
    pub fn validate(&self) -> bool {
        [&self.project, &self.service].into_iter().all(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.as_bytes()[0].is_ascii_alphanumeric()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stack {
    #[serde(default)]
    pub kind: StackKind,
    pub environment: String,
    pub name: String,
    pub services: Vec<StackService>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackKind {
    #[default]
    Compose,
    Swarm,
}
impl StackKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Compose => "Compose",
            Self::Swarm => "Swarm",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StackService {
    #[serde(default)]
    pub swarm: Option<SwarmService>,
    pub name: String,
    pub containers: Vec<Container>,
}
impl Stack {
    pub fn from_snapshot(snapshot: &Snapshot) -> Vec<Self> {
        if !snapshot.capabilities.stacks {
            return vec![];
        }
        let mut groups = BTreeMap::<String, BTreeMap<String, Vec<Container>>>::new();
        for c in &snapshot.containers {
            let Some(m) = &c.compose else { continue };
            if !m.validate()
                || c.key.environment != snapshot.environment
                || c.key.provider != ProviderKind::Docker
            {
                continue;
            }
            groups
                .entry(m.project.clone())
                .or_default()
                .entry(m.service.clone())
                .or_default()
                .push(c.clone());
        }
        let mut stacks: Vec<Self> = groups
            .into_iter()
            .map(|(name, services)| Self {
                kind: StackKind::Compose,
                environment: snapshot.environment.clone(),
                name,
                services: services
                    .into_iter()
                    .map(|(name, mut containers)| {
                        containers.sort_by(|a, b| a.key.id.cmp(&b.key.id));
                        StackService {
                            name,
                            containers,
                            swarm: None,
                        }
                    })
                    .collect(),
            })
            .collect();
        let mut swarm = BTreeMap::<String, Vec<StackService>>::new();
        for service in &snapshot.swarm.services {
            if service.key.environment != snapshot.environment
                || service.command(SwarmAction::Remove).is_err()
            {
                continue;
            }
            swarm
                .entry(service.stack.clone())
                .or_default()
                .push(StackService {
                    name: service.key.id.clone(),
                    containers: snapshot
                        .containers
                        .iter()
                        .filter(|c| {
                            c.key.environment == snapshot.environment
                                && c.key.provider == ProviderKind::Docker
                                && service.tasks.iter().any(|t| {
                                    !matches!(t.desired_state.as_str(), "shutdown" | "remove")
                                        && t.container_id.as_deref() == Some(&c.key.incarnation)
                                })
                        })
                        .cloned()
                        .collect(),
                    swarm: Some(service.clone()),
                });
        }
        stacks.extend(swarm.into_iter().map(|(name, services)| Self {
            environment: snapshot.environment.clone(),
            name,
            kind: StackKind::Swarm,
            services,
        }));
        stacks
    }
    pub fn identity(&self) -> String {
        format!("{}:{}", self.kind.label(), self.name)
    }
    pub fn containers(&self) -> Vec<&Container> {
        self.services.iter().flat_map(|s| &s.containers).collect()
    }
    /// A service selection is explicit: a missing/duplicate name must never
    /// silently broaden a command to the entire project.
    pub fn commands_for_services(
        &self,
        action: StackAction,
        names: &[String],
    ) -> Result<Vec<Command>, ManagementError> {
        let count = names.len();
        let names: BTreeSet<_> = names.iter().collect();
        if names.len() != count
            || names.is_empty()
            || names
                .iter()
                .any(|name| !self.services.iter().any(|s| &s.name == *name))
        {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "所选服务已改变，请刷新",
            ));
        }
        let mut scoped = self.clone();
        scoped.services.retain(|s| names.contains(&s.name));
        scoped.commands(action)
    }
    /// Capture immutable member identities. Tasks reconcile independently; this is not Compose up/down.
    pub fn commands(&self, action: StackAction) -> Result<Vec<Command>, ManagementError> {
        if self.kind == StackKind::Swarm {
            let action = match action {
                StackAction::Stop => SwarmAction::Scale { replicas: 0 },
                StackAction::Restart => SwarmAction::Restart,
                StackAction::RemoveServices => SwarmAction::Remove,
                _ => {
                    return Err(ManagementError::new(
                        ErrorKind::Unsupported,
                        "请通过副本数启动 Swarm 服务",
                    ));
                }
            };
            let mut seen = BTreeSet::new();
            return self
                .services
                .iter()
                .map(|s| {
                    let service = s.swarm.as_ref().ok_or_else(|| {
                        ManagementError::new(ErrorKind::InvalidData, "服务身份缺失")
                    })?;
                    if service.key.environment != self.environment
                        || service.stack != self.name
                        || !seen.insert(&service.key.incarnation)
                    {
                        return Err(ManagementError::new(
                            ErrorKind::Conflict,
                            "Swarm 服务身份不一致",
                        ));
                    }
                    service.command(action)
                })
                .collect();
        }
        if action == StackAction::RemoveServices {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "Compose 请使用移除已停止容器",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut commands = vec![];
        for c in self.containers() {
            if c.key.environment != self.environment
                || c.key.provider != ProviderKind::Docker
                || c.compose
                    .as_ref()
                    .is_none_or(|m| m.project != self.name || !m.validate())
                || !seen.insert(c.key.incarnation.clone())
            {
                return Err(ManagementError::new(
                    ErrorKind::Conflict,
                    "Stack 成员身份不一致，请刷新",
                ));
            }
            // One-off jobs are visible but are never restarted by a group action.
            if c.compose.as_ref().is_some_and(|m| m.one_off) && action != StackAction::RemoveStopped
            {
                continue;
            }
            let command = match (action, &c.state) {
                (StackAction::Start, ContainerState::Stopped) => Command::Lifecycle {
                    key: c.key.clone(),
                    action: LifecycleAction::Start,
                },
                (StackAction::Stop, ContainerState::Running) => Command::Lifecycle {
                    key: c.key.clone(),
                    action: LifecycleAction::Stop,
                },
                (StackAction::Restart, ContainerState::Running) => Command::Lifecycle {
                    key: c.key.clone(),
                    action: LifecycleAction::Restart,
                },
                (StackAction::RemoveStopped, ContainerState::Stopped) => {
                    Command::DeleteContainer { key: c.key.clone() }
                }
                _ => continue,
            };
            command.validate()?;
            commands.push(command);
        }
        Ok(commands)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackAction {
    Start,
    Stop,
    Restart,
    RemoveStopped,
    RemoveServices,
}
impl StackAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "启动",
            Self::Stop => "停止",
            Self::Restart => "重启",
            Self::RemoveStopped => "移除已停止容器",
            Self::RemoveServices => "移除服务",
        }
    }
}
