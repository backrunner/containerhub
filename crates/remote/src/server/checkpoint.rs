use super::*;
use containerhub_core::{CommandResult, Container, CreationCheckpoint, OperationProgress};

pub(super) struct Journal {
    pub store: Arc<BindingStore>,
    pub auth: Authorization,
    pub operation: RemoteOperation,
}
#[async_trait::async_trait]
impl CreationCheckpoint for Journal {
    async fn save(&self, container: &Container) -> std::result::Result<(), ManagementError> {
        containerhub_core::validate_created(&self.operation.request.command, container)?;
        let mut operation = self.operation.clone();
        operation.state = OperationState::Running;
        operation.result = Some(CommandResult::Container(Box::new(container.clone())));
        operation.progress = Some(OperationProgress::Starting);
        self.store
            .finish_operation(
                &operation.request.request_id,
                &serde_json::to_string(&operation).expect("operation"),
            )
            .map_err(|_| {
                ManagementError::new(ErrorKind::Unavailable, "Agent 无法保存创建结果，未继续启动")
            })?;
        // Grants can be revoked while creation is running. Persist its result even if start is now forbidden.
        authorize_command(&self.store, &self.auth, &operation.request.command)
            .map_err(|error| ManagementError::new(ErrorKind::PermissionDenied, error.to_string()))
    }
}
