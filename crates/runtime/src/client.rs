use containerhub_core::*;
use std::sync::{Arc, RwLock};
pub struct ClientSlot(RwLock<Arc<dyn ManagementClient>>);
impl ClientSlot {
    pub fn new(client: Arc<dyn ManagementClient>) -> Self {
        Self(RwLock::new(client))
    }
    pub fn replace(&self, client: Arc<dyn ManagementClient>) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = client;
    }
    fn current(&self) -> Arc<dyn ManagementClient> {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
}
#[async_trait::async_trait]
impl ManagementClient for ClientSlot {
    fn prepare_execution(&self, c: &Command) -> Result<Option<ExecutionReceipt>, ManagementError> {
        self.current().prepare_execution(c)
    }
    async fn execute_prepared(
        &self,
        c: &Command,
        r: Option<&ExecutionReceipt>,
        x: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        self.current().execute_prepared(c, r, x).await
    }
    async fn resume_execution(
        &self,
        c: &Command,
        r: &ExecutionReceipt,
        x: &ExecutionControl,
    ) -> Result<RecoveredExecution, ManagementError> {
        self.current().resume_execution(c, r, x).await
    }
    async fn host_snapshot(&self) -> Result<HostSnapshot, ManagementError> {
        self.current().host_snapshot().await
    }
    async fn diagnostics(&self) -> Result<RuntimeDiagnostics, ManagementError> {
        self.current().diagnostics().await
    }
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        self.current().snapshot().await
    }
    async fn resources(&self, k: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        self.current().resources(k).await
    }
    async fn inspect_image(&self, e: &str, r: &str) -> Result<ImageDetails, ManagementError> {
        self.current().inspect_image(e, r).await
    }
    async fn logs(&self, k: &ResourceKey, s: LogSource) -> Result<LogSnapshot, ManagementError> {
        self.current().logs(k, s).await
    }
    async fn lifecycle(&self, k: &ResourceKey, a: LifecycleAction) -> Result<(), ManagementError> {
        self.current().lifecycle(k, a).await
    }
    async fn execute(&self, c: &Command) -> Result<CommandResult, ManagementError> {
        self.current().execute(c).await
    }
    async fn execute_controlled(
        &self,
        c: &Command,
        x: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        self.current().execute_controlled(c, x).await
    }
    async fn inspect_container(
        &self,
        k: &ResourceKey,
    ) -> Result<ContainerDetails, ManagementError> {
        self.current().inspect_container(k).await
    }
    async fn stats(&self, k: &ResourceKey) -> Result<ContainerStats, ManagementError> {
        self.current().stats(k).await
    }
}
