use crate::{ErrorKind, ManagementError};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Confirmed phases only. Runtime text is never forwarded as progress or persisted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationProgress {
    Waiting,
    Fetching,
    Unpacking,
    Verifying,
    Stopping,
    Replacing,
    Creating,
    Starting,
}
impl OperationProgress {
    pub fn label(self) -> &'static str {
        match self {
            Self::Waiting => "等待运行时",
            Self::Fetching => "下载镜像",
            Self::Unpacking => "解包镜像",
            Self::Verifying => "核对结果",
            Self::Stopping => "停止原容器",
            Self::Replacing => "替换原容器",
            Self::Creating => "创建容器",
            Self::Starting => "启动已创建容器",
        }
    }
}

#[derive(Clone, Default)]
pub struct ExecutionControl(Arc<Control>);
#[derive(Default)]
struct Control {
    cancelled: AtomicBool,
    secrets: crate::SecretEnvironment,
    progress: Mutex<Option<OperationProgress>>,
    checkpoint: Option<Arc<dyn crate::CreationCheckpoint>>,
    created: Mutex<Option<crate::Container>>,
    saved: Mutex<Option<(crate::ResourceKey, crate::ContainerState)>>,
}
impl ExecutionControl {
    pub fn with_checkpoint(
        secrets: crate::SecretEnvironment,
        checkpoint: Arc<dyn crate::CreationCheckpoint>,
    ) -> Self {
        Self(Arc::new(Control {
            secrets,
            checkpoint: Some(checkpoint),
            ..Default::default()
        }))
    }
    pub fn has_checkpoint(&self) -> bool {
        self.0.checkpoint.is_some()
    }
    pub fn created(&self) -> Option<crate::Container> {
        self.0.created.lock().expect("created").clone()
    }
    /// Retain the confirmed identity even if the durable write fails; never start without its ACK.
    pub async fn checkpoint_created(
        &self,
        container: &crate::Container,
    ) -> Result<(), ManagementError> {
        {
            let mut created = self.0.created.lock().expect("created");
            if let Some(old) = &*created
                && old.key != container.key
            {
                return Err(ManagementError::new(
                    ErrorKind::Conflict,
                    "创建结果身份发生变化",
                ));
            }
            *created = Some(container.clone());
        }
        let identity = (container.key.clone(), container.state.clone());
        if self.0.saved.lock().expect("saved checkpoint").as_ref() == Some(&identity) {
            return Ok(());
        }
        if let Some(checkpoint) = &self.0.checkpoint {
            checkpoint.save(container).await?;
        }
        *self.0.saved.lock().expect("saved checkpoint") = Some(identity);
        Ok(())
    }
    pub fn with_secrets(secrets: crate::SecretEnvironment) -> Self {
        Self(Arc::new(Control {
            secrets,
            ..Default::default()
        }))
    }
    pub fn secrets(&self) -> &crate::SecretEnvironment {
        &self.0.secrets
    }
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }
    pub fn check_before_start(&self) -> Result<(), ManagementError> {
        if self.is_cancelled() {
            Err(ManagementError::new(
                ErrorKind::Cancelled,
                "已取消，尚未执行运行时命令",
            ))
        } else {
            Ok(())
        }
    }
    pub fn report(&self, progress: OperationProgress) {
        *self.0.progress.lock().expect("progress") = Some(progress);
    }
    pub fn progress(&self) -> Option<OperationProgress> {
        *self.0.progress.lock().expect("progress")
    }
}
