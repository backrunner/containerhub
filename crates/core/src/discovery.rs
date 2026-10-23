use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFeature {
    Stacks,
    StartRuntime,
    Containers,
    Images,
    Volumes,
    Networks,
    Logs,
    BootLogs,
    InspectContainer,
    InspectImage,
    Stats,
    Lifecycle,
    PullImage,
    CancelPull,
    CreateContainer,
    AdvancedCreate,
    SecretEnvironment,
    StartupCommand,
    CreateAndStart,
    RebuildPorts,
    DeleteContainer,
    DeleteImage,
}
impl RuntimeFeature {
    pub const ALL: [Self; 22] = [
        Self::Stacks,
        Self::StartRuntime,
        Self::Containers,
        Self::Images,
        Self::Volumes,
        Self::Networks,
        Self::Logs,
        Self::BootLogs,
        Self::InspectContainer,
        Self::InspectImage,
        Self::Stats,
        Self::Lifecycle,
        Self::PullImage,
        Self::CancelPull,
        Self::CreateContainer,
        Self::AdvancedCreate,
        Self::SecretEnvironment,
        Self::StartupCommand,
        Self::CreateAndStart,
        Self::RebuildPorts,
        Self::DeleteContainer,
        Self::DeleteImage,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Stacks => "Stack 管理",
            Self::StartRuntime => "启动运行时服务",
            Self::Containers => "容器列表",
            Self::Images => "镜像列表",
            Self::Volumes => "存储卷列表",
            Self::Networks => "网络列表",
            Self::Logs => "应用日志",
            Self::BootLogs => "启动日志",
            Self::InspectContainer => "容器检查",
            Self::InspectImage => "镜像检查",
            Self::Stats => "实时统计",
            Self::Lifecycle => "启动、停止与重启",
            Self::PullImage => "拉取镜像",
            Self::CancelPull => "取消拉取",
            Self::CreateContainer => "创建容器",
            Self::AdvancedCreate => "高级创建选项",
            Self::SecretEnvironment => "秘密环境变量",
            Self::StartupCommand => "自定义启动命令",
            Self::CreateAndStart => "创建后立即启动",
            Self::RebuildPorts => "重建端口映射",
            Self::DeleteContainer => "删除容器",
            Self::DeleteImage => "删除镜像",
        }
    }
    pub fn writes(self) -> bool {
        matches!(
            self,
            Self::StartRuntime
                | Self::Lifecycle
                | Self::PullImage
                | Self::CancelPull
                | Self::CreateContainer
                | Self::StartupCommand
                | Self::CreateAndStart
                | Self::SecretEnvironment
                | Self::AdvancedCreate
                | Self::RebuildPorts
                | Self::DeleteContainer
                | Self::DeleteImage
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityCheck {
    pub feature: RuntimeFeature,
    pub available: bool,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeDiagnostics {
    pub environment: String,
    pub cli_path: Option<String>,
    pub cli_version: Option<String>,
    pub observed_at: u64,
    /// A capability describes the installed contract; service reachability is independent.
    pub service_error: Option<ManagementError>,
    pub checks: Vec<CapabilityCheck>,
}
impl RuntimeDiagnostics {
    pub fn supports(&self, feature: RuntimeFeature) -> bool {
        self.checks
            .iter()
            .any(|c| c.feature == feature && c.available)
    }
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            stacks: self.supports(RuntimeFeature::Stacks),
            inspect_container: self.supports(RuntimeFeature::InspectContainer),
            rebuild_ports: self.supports(RuntimeFeature::RebuildPorts),
            inspect_image: self.supports(RuntimeFeature::InspectImage),
            stats: self.supports(RuntimeFeature::Stats),
            pull_image: self.supports(RuntimeFeature::PullImage),
            cancel_pull: self.supports(RuntimeFeature::CancelPull),
            create_container: self.supports(RuntimeFeature::CreateContainer),
            create_advanced: self.supports(RuntimeFeature::AdvancedCreate),
            create_secrets: self.supports(RuntimeFeature::SecretEnvironment),
            create_startup: self.supports(RuntimeFeature::StartupCommand),
            create_and_start: self.supports(RuntimeFeature::CreateAndStart),
            delete_container: self.supports(RuntimeFeature::DeleteContainer),
            delete_image: self.supports(RuntimeFeature::DeleteImage),
        }
    }
    pub fn require(&self, feature: RuntimeFeature) -> Result<(), ManagementError> {
        if self.supports(feature) {
            Ok(())
        } else {
            Err(ManagementError::new(
                ErrorKind::Unsupported,
                self.checks
                    .iter()
                    .find(|c| c.feature == feature)
                    .and_then(|c| c.reason.clone())
                    .unwrap_or_else(|| format!("当前运行时不支持{}", feature.label())),
            ))
        }
    }
}
