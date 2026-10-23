use containerhub_core::*;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt};

async fn bounded(stream: impl AsyncRead + Unpin) -> Result<Vec<u8>, ManagementError> {
    const LIMIT: u64 = 16 * 1024 * 1024;
    let mut output = vec![];
    stream
        .take(LIMIT + 1)
        .read_to_end(&mut output)
        .await
        .map_err(|_| ManagementError::new(ErrorKind::Unavailable, "无法读取 Docker 响应"))?;
    if output.len() > LIMIT as usize {
        return Err(ManagementError::new(
            ErrorKind::InvalidData,
            "Docker 响应超过 16 MiB",
        ));
    }
    Ok(output)
}
pub async fn run(
    binary: &Path,
    context: &str,
    args: &[&str],
    write: bool,
) -> Result<Vec<u8>, ManagementError> {
    // Explicit context, no shell; inherited host overrides cannot redirect a request.
    let mut child = tokio::process::Command::new(binary)
        .args(["--context", context])
        .args(args)
        .env_remove("DOCKER_HOST")
        .env_remove("DOCKER_CONTEXT")
        .env_remove("DOCKER_API_VERSION")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| {
            ManagementError::new(
                ErrorKind::NotInstalled,
                "未找到 Docker CLI，请检查安装与路径",
            )
        })?;
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let result = tokio::time::timeout(Duration::from_secs(if write { 90 } else { 15 }), async {
        let (out, _, status) = tokio::try_join!(bounded(out), bounded(err), async {
            child
                .wait()
                .await
                .map_err(|_| ManagementError::new(ErrorKind::Unavailable, "Docker 进程中断"))
        })?;
        if !status.success() {
            return Err(ManagementError::new(
                if write {
                    ErrorKind::UnknownOutcome
                } else {
                    ErrorKind::Unavailable
                },
                if write {
                    "Docker 操作未确认，请刷新核对实际状态"
                } else {
                    "无法读取 Docker，请检查所选 context 和 Engine 状态"
                },
            ));
        }
        Ok(out)
    })
    .await;
    match result {
        Ok(Ok(bytes)) => Ok(bytes),
        failure => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let error = match failure {
                Ok(Err(e)) => e,
                _ => ManagementError::new(ErrorKind::Timeout, "Docker 响应超时"),
            };
            Err(if write {
                ManagementError::new(
                    ErrorKind::UnknownOutcome,
                    "Docker 操作结果待核对；未自动重试",
                )
            } else {
                error
            })
        }
    }
}
