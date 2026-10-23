use crate::protocol::*;
use containerhub_core::*;
use containerhub_management::service::OperationService;

pub async fn dispatch(
    request: Request,
    provider: &dyn ManagementClient,
    operations: &OperationService,
) -> Result<serde_json::Value, ManagementError> {
    let value = match request {
        Request::Hello => {
            return Ok(
                serde_json::json!({"protocol":VERSION,"product":"containerhub","transport":"stdio","max_frame_bytes":MAX_FRAME}),
            );
        }
        Request::InspectContainer { key } => {
            serde_json::to_value(provider.inspect_container(&key).await?)
        }
        Request::Cancel { id } => serde_json::to_value(operations.cancel(id).await?),
        Request::Reconcile { id } => serde_json::to_value(operations.reconcile(id).await?),
        Request::HostSnapshot => serde_json::to_value(provider.host_snapshot().await?),
        Request::Diagnostics => serde_json::to_value(provider.diagnostics().await?),
        Request::Stacks => serde_json::to_value(Stack::from_snapshot(&provider.snapshot().await?)),
        Request::StackCommands {
            kind,
            environment,
            name,
            action,
        } => {
            let snapshot = provider.snapshot().await?;
            if snapshot.environment != environment {
                return Err(ManagementError::new(
                    ErrorKind::Conflict,
                    "Stack 环境不匹配",
                ));
            }
            let stack = Stack::from_snapshot(&snapshot)
                .into_iter()
                .find(|s| s.name == name && s.kind == kind)
                .ok_or_else(|| ManagementError::new(ErrorKind::NotFound, "Stack 已不存在"))?;
            serde_json::to_value(stack.commands(action)?)
        }
        Request::Snapshot => serde_json::to_value(provider.snapshot().await?),
        Request::Resources { kind } => serde_json::to_value(provider.resources(kind).await?),
        Request::InspectImage {
            environment,
            reference,
        } => serde_json::to_value(provider.inspect_image(&environment, &reference).await?),
        Request::ExportLogs { key, source, query } => {
            serde_json::to_value(provider.logs(&key, source).await?.export(&query)?)
        }
        Request::Logs { key, source } => serde_json::to_value(provider.logs(&key, source).await?),
        Request::Stats { key } => serde_json::to_value(provider.stats(&key).await?),
        Request::Submit {
            operation_id,
            command,
            secrets,
        } => serde_json::to_value(
            operations
                .submit_with_secrets(operation_id, command, secrets)
                .await?,
        ),
        Request::Operation { id } => serde_json::to_value(operations.get(id)?),
        Request::Operations => serde_json::to_value(operations.recent()?),
    };
    value.map_err(|_| ManagementError::new(ErrorKind::InvalidData, "响应无法序列化"))
}
