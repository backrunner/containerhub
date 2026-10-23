use crate::*;

/// Operational state, deliberately distinct from container health checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackServiceStatus {
    Running,
    Stopped,
    Partial,
    Unhealthy,
    Scheduling,
    Updating,
    UpdatePaused,
    Incomplete,
    Unknown,
    Job,
}
impl StackServiceStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "运行中",
            Self::Stopped => "已停止",
            Self::Partial => "部分运行",
            Self::Unhealthy => "不健康",
            Self::Scheduling => "调度中",
            Self::Updating => "更新中",
            Self::UpdatePaused => "更新暂停",
            Self::Incomplete => "采集不完整",
            Self::Unknown => "状态未知",
            Self::Job => "一次性任务",
        }
    }
    pub fn needs_attention(self) -> bool {
        matches!(
            self,
            Self::Partial
                | Self::Unhealthy
                | Self::Scheduling
                | Self::UpdatePaused
                | Self::Incomplete
                | Self::Unknown
        )
    }
}
impl StackService {
    pub fn status(&self) -> StackServiceStatus {
        use StackServiceStatus::*;
        if let Some(s) = &self.swarm {
            if s.update_state
                .as_deref()
                .is_some_and(|s| s.contains("paused"))
            {
                return UpdatePaused;
            }
            if s.tasks_truncated {
                return Incomplete;
            }
            if s.mode == SwarmMode::Job {
                return Job;
            }
            if s.mode == SwarmMode::Unknown {
                return Unknown;
            }
            if matches!(
                s.update_state.as_deref(),
                Some("updating" | "rollback_started")
            ) {
                return Updating;
            }
            let desired = s.desired_replicas.unwrap_or_else(|| {
                s.tasks
                    .iter()
                    .filter(|t| t.desired_state == "running")
                    .count() as u64
            });
            if desired == 0 {
                return if s.running() > 0 {
                    Updating
                } else if s.mode == SwarmMode::Replicated {
                    Stopped
                } else {
                    Unknown
                };
            }
            return if (s.running() as u64) < desired {
                Scheduling
            } else if (s.running() as u64) > desired {
                Updating
            } else {
                Running
            };
        }
        let members: Vec<_> = self
            .containers
            .iter()
            .filter(|c| !c.compose.as_ref().is_some_and(|m| m.one_off))
            .collect();
        if members.is_empty() {
            return Job;
        }
        if members
            .iter()
            .any(|c| c.state.is_running() && c.health == ContainerHealth::Unhealthy)
        {
            return Unhealthy;
        }
        if members
            .iter()
            .any(|c| matches!(c.state, ContainerState::Unknown(_)))
        {
            return Unknown;
        }
        let running = members.iter().filter(|c| c.state.is_running()).count();
        if running == 0 {
            Stopped
        } else if running == members.len() {
            Running
        } else {
            Partial
        }
    }
    pub fn running_count(&self) -> usize {
        self.swarm.as_ref().map(|s| s.running()).unwrap_or_else(|| {
            self.containers
                .iter()
                .filter(|c| c.state.is_running())
                .count()
        })
    }
    pub fn image(&self) -> &str {
        self.swarm
            .as_ref()
            .map(|s| s.image.as_str())
            .or_else(|| self.containers.first().map(|c| c.image.as_str()))
            .unwrap_or("—")
    }
    pub fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        self.name.to_lowercase().contains(&q)
            || self.image().to_lowercase().contains(&q)
            || self.containers.iter().any(|c| c.matches(&q))
    }
}
