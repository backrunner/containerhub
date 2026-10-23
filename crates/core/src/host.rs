//! Host telemetry is independent of container inventory and never inferred from it.
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HostSnapshot {
    pub environment: String,
    pub observed_at: u64,
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub architecture: String,
    pub processor: Option<String>,
    pub logical_cpus: u32,
    pub uptime_seconds: u64,
    /// All logical CPUs normalized to 0..100, not per-container CPU accounting.
    pub cpu_percent: Option<f64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    /// One writable system filesystem; APFS volumes are not summed.
    pub disk_path: Option<String>,
    pub disk_used_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub disk_read_bytes_per_second: Option<f64>,
    pub disk_write_bytes_per_second: Option<f64>,
    /// Non-loopback interfaces; no addresses or process information are included.
    pub network_receive_bytes_per_second: Option<f64>,
    pub network_send_bytes_per_second: Option<f64>,
    pub sample: bool,
}
impl HostSnapshot {
    pub fn validate(&self) -> Result<(), ManagementError> {
        let rates = [
            self.cpu_percent,
            self.disk_read_bytes_per_second,
            self.disk_write_bytes_per_second,
            self.network_receive_bytes_per_second,
            self.network_send_bytes_per_second,
        ];
        if self.environment.is_empty()
            || rates
                .into_iter()
                .flatten()
                .any(|v| !v.is_finite() || v < 0.)
            || self.cpu_percent.is_some_and(|v| v > 100.)
            || matches!((self.memory_used_bytes,self.memory_total_bytes),(Some(u),Some(t)) if u>t)
            || matches!((self.disk_used_bytes,self.disk_total_bytes),(Some(u),Some(t)) if u>t)
        {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "主机采样数据无效",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerHealth {
    Healthy,
    Unhealthy,
    Starting,
    Unconfigured,
    Unsupported,
    #[default]
    Unknown,
}
impl ContainerHealth {
    pub fn label(self) -> &'static str {
        match self {
            Self::Healthy => "健康",
            Self::Unhealthy => "不健康",
            Self::Starting => "检查中",
            Self::Unconfigured => "未配置",
            Self::Unsupported => "不支持",
            Self::Unknown => "未知",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContainerOverview {
    pub running: usize,
    pub stopped: usize,
    pub unknown: usize,
    pub healthy: usize,
    pub unhealthy: usize,
    pub starting: usize,
    pub unconfigured: usize,
    pub unsupported: usize,
    pub health_unknown: usize,
}
impl ContainerOverview {
    pub fn from_containers(containers: &[Container]) -> Self {
        let mut summary = Self::default();
        for c in containers {
            match &c.state {
                ContainerState::Running => summary.running += 1,
                ContainerState::Stopped => {
                    summary.stopped += 1;
                    continue;
                }
                ContainerState::Unknown(_) => {
                    summary.unknown += 1;
                    continue;
                }
            }
            match c.health {
                ContainerHealth::Healthy => summary.healthy += 1,
                ContainerHealth::Unhealthy => summary.unhealthy += 1,
                ContainerHealth::Starting => summary.starting += 1,
                ContainerHealth::Unconfigured => summary.unconfigured += 1,
                ContainerHealth::Unsupported => summary.unsupported += 1,
                ContainerHealth::Unknown => summary.health_unknown += 1,
            }
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_and_missing_metrics_remain_distinct() {
        let mut sample = HostSnapshot {
            environment: "test".into(),
            ..Default::default()
        };
        assert!(sample.validate().is_ok());
        for value in [f64::NAN, f64::INFINITY, -1., 100.1] {
            sample.cpu_percent = Some(value);
            assert!(sample.validate().is_err());
        }
        sample.cpu_percent = None;
        sample.memory_used_bytes = Some(2);
        sample.memory_total_bytes = Some(1);
        assert!(sample.validate().is_err());
    }
    #[tokio::test]
    async fn only_running_containers_contribute_to_health() {
        let mut containers = fake::FakeProvider::default()
            .snapshot()
            .await
            .unwrap()
            .containers;
        for container in &mut containers {
            container.health = ContainerHealth::Healthy;
        }
        let summary = ContainerOverview::from_containers(&containers);
        assert_eq!(summary.healthy, summary.running);
        assert!(summary.stopped > 0);
        let running = containers
            .iter_mut()
            .find(|c| c.state.is_running())
            .unwrap();
        running.health = ContainerHealth::Unsupported;
        let summary = ContainerOverview::from_containers(&containers);
        assert_eq!(summary.healthy + 1, summary.running);
        assert_eq!(summary.unsupported, 1);
    }
}
