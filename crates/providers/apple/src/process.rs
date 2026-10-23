use containerhub_core::{
    ErrorKind, ExecutionControl, ManagementError, OperationProgress, SecretEnvironment,
};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};

const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
async fn bounded(stream: impl AsyncRead + Unpin) -> Result<Vec<u8>, ManagementError> {
    bounded_progress(stream, None).await
}
async fn bounded_progress(
    mut stream: impl AsyncRead + Unpin,
    control: Option<&ExecutionControl>,
) -> Result<Vec<u8>, ManagementError> {
    let mut output = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|_| ManagementError::new(ErrorKind::Unavailable, "无法读取运行时响应"))?;
        if n == 0 {
            return Ok(output);
        }
        if output.len() + n > OUTPUT_LIMIT {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "运行时响应超过 8 MiB 上限，已停止读取",
            ));
        }
        output.extend_from_slice(&chunk[..n]);
        if let Some(control) = control {
            let tail = &output[output.len().saturating_sub(n + 64)..];
            let text = String::from_utf8_lossy(tail);
            if text.contains("Unpacking image") {
                control.report(OperationProgress::Unpacking);
            } else if text.contains("Fetching image")
                && control.progress() != Some(OperationProgress::Unpacking)
            {
                control.report(OperationProgress::Fetching);
            }
        }
    }
}
pub async fn run(path: &Path, args: &[&str], write: bool) -> Result<Vec<u8>, ManagementError> {
    run_with_timeout(
        path,
        args,
        write,
        Duration::from_secs(if write { 45 } else { 15 }),
    )
    .await
}
pub async fn run_with_timeout(
    path: &Path,
    args: &[&str],
    write: bool,
    deadline: Duration,
) -> Result<Vec<u8>, ManagementError> {
    run_controlled(path, args, write, deadline, None).await
}
pub async fn cancelled(control: &ExecutionControl) {
    while !control.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
pub async fn run_controlled(
    path: &Path,
    args: &[&str],
    write: bool,
    deadline: Duration,
    control: Option<&ExecutionControl>,
) -> Result<Vec<u8>, ManagementError> {
    run_input(
        path,
        args,
        write,
        deadline,
        control,
        &SecretEnvironment::default(),
    )
    .await
}
pub async fn run_with_input(
    path: &Path,
    args: &[&str],
    write: bool,
    deadline: Duration,
    secrets: &SecretEnvironment,
) -> Result<Vec<u8>, ManagementError> {
    run_input(path, args, write, deadline, None, secrets).await
}
async fn run_input(
    path: &Path,
    args: &[&str],
    write: bool,
    deadline: Duration,
    control: Option<&ExecutionControl>,
    secrets: &SecretEnvironment,
) -> Result<Vec<u8>, ManagementError> {
    if let Some(control) = control {
        control.check_before_start()?;
    }
    let input = secrets.stdin_bytes()?;
    let mut command = Command::new(path);
    command
        .args(args)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env_remove("CONTAINER_DEBUG")
        .env_remove("CONTAINER_DEBUG_LAUNCHD_LABEL")
        .stdin(if secrets.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|_| {
        ManagementError::new(
            ErrorKind::NotInstalled,
            "无法运行 Apple container；请检查安装路径与执行权限",
        )
    })?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let work = async {
        let send_input = async move {
            if let Some(mut stdin) = stdin {
                stdin.write_all(&input).await.map_err(|_| {
                    ManagementError::new(
                        ErrorKind::UnknownOutcome,
                        "运行时秘密输入中断；请核对实际资源状态",
                    )
                })?;
                // Drop closes the pipe and lets envFile.readToEnd finish.
            }
            Ok::<(), ManagementError>(())
        };
        let (out, err, (), status) = tokio::try_join!(
            bounded(stdout),
            bounded_progress(stderr, control),
            send_input,
            async {
                child.wait().await.map_err(|_| {
                    ManagementError::new(ErrorKind::Unavailable, "无法读取命令退出状态")
                })
            }
        )?;
        if !status.success() {
            // Never surface raw command output: it may contain secrets or paths from the runtime.
            let err = String::from_utf8_lossy(&err).to_lowercase();
            let (kind, message) = if err.contains("permission") || err.contains("denied") {
                (
                    ErrorKind::PermissionDenied,
                    "运行时拒绝访问，请检查当前用户权限",
                )
            } else if err.contains("not found") || err.contains("does not exist") {
                (ErrorKind::NotFound, "目标资源不存在，请刷新列表")
            } else {
                (
                    ErrorKind::Unavailable,
                    "运行时命令失败；请检查 container system status，确认服务和目标资源可用",
                )
            };
            return Err(ManagementError::new(kind, message));
        }
        Ok(out)
    };
    let result = {
        let cancel = async {
            if let Some(control) = control {
                cancelled(control).await
            } else {
                std::future::pending::<()>().await
            }
        };
        tokio::select! {
            result = tokio::time::timeout(deadline, work) => result,
            _ = cancel => Ok(Err(ManagementError::new(ErrorKind::UnknownOutcome, "已停止拉取客户端；后台可能仍在处理，请刷新镜像列表核对，未回滚已下载内容"))),
        }
    };
    match result {
        Ok(Ok(out)) => Ok(out),
        Ok(Err(e)) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err(e)
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err(ManagementError::new(
                if write {
                    ErrorKind::UnknownOutcome
                } else {
                    ErrorKind::Timeout
                },
                if write {
                    "等待操作超时；请检查 macOS 访问权限弹窗。客户端已退出，运行时结果仍需刷新核对"
                } else {
                    "运行时响应超时，请刷新重试"
                },
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn output_is_bounded_and_not_silently_truncated() {
        let data = vec![b'x'; OUTPUT_LIMIT + 1];
        assert_eq!(
            bounded(data.as_slice()).await.unwrap_err().kind,
            ErrorKind::InvalidData
        );
        assert_eq!(bounded(&b"ok"[..]).await.unwrap(), b"ok");
    }
}
