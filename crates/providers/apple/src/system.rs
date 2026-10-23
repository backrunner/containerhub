use super::*;

impl AppleProvider {
    pub(super) async fn start_runtime(
        &self,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        self.start_runtime_with_timeout(control, Duration::from_secs(40))
            .await
    }
    async fn start_runtime_with_timeout(
        &self,
        control: &ExecutionControl,
        timeout: Duration,
    ) -> Result<CommandResult, ManagementError> {
        let _guard = self.runtime_starts.lock().await;
        control.check_before_start()?;
        // Detect changes to the installed CLI before issuing a host-level action.
        self.discover(true)
            .await
            .require(RuntimeFeature::StartRuntime)?;
        match self.service_status().await {
            Ok(()) => return Ok(CommandResult::Confirmed),
            Err(error)
                if matches!(
                    error.kind,
                    ErrorKind::PermissionDenied | ErrorKind::InvalidData
                ) =>
            {
                return Err(error);
            }
            Err(_) => {}
        }
        control.check_before_start()?;
        control.report(OperationProgress::Starting);
        // Never answer an installation prompt or download a kernel implicitly.
        // First-time setup remains an explicit action in the user's terminal.
        let result = process::run_with_timeout(
            self.binary.as_ref().expect("discovery checked CLI"),
            &[
                "system",
                "start",
                "--disable-kernel-install",
                "--timeout",
                "30",
            ],
            true,
            timeout,
        )
        .await;
        control.report(OperationProgress::Verifying);
        if self.service_status().await.is_ok() {
            return Ok(CommandResult::Confirmed);
        }
        if let Err(error) = result
            && matches!(
                error.kind,
                ErrorKind::PermissionDenied | ErrorKind::NotInstalled
            )
        {
            return Err(error);
        }
        Err(ManagementError::new(
            ErrorKind::UnknownOutcome,
            "启动请求已结束，但服务仍不可连接。可重新检查或在任务中心核对；首次安装请在终端运行 container system start 完成内核安装，macOS 权限需手动处理。未自动重试。",
        ))
    }
}

#[cfg(all(test, unix))]
mod tests;
