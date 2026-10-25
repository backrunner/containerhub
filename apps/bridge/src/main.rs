//! Versioned bounded stdio bridge. It opens no listening socket and never invokes a shell.
mod dispatch;
mod protocol;
use anyhow::{Result, ensure};
use clap::Parser;
use containerhub_core::{ErrorKind, ManagementError};
use containerhub_management::service::OperationService;
use containerhub_runtime::Runtime;
use protocol::*;
use std::{path::PathBuf, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Mutex, Semaphore},
    task::JoinSet,
};
use zeroize::{Zeroize, Zeroizing};

#[derive(Parser)]
#[command(about = "ContainerHub shared management service over bounded JSON stdio")]
struct Args {
    #[arg(long)]
    state: PathBuf,
    #[arg(long, conflicts_with_all = ["demo", "remote_profile"])]
    apple_binary: Option<PathBuf>,
    #[arg(long, conflicts_with_all=["demo","remote_profile","apple_binary"])]
    docker_context: Option<String>,
    #[arg(long, requires = "docker_context")]
    docker_binary: Option<PathBuf>,
    #[arg(long, conflicts_with = "remote_profile")]
    demo: bool,
    #[arg(long)]
    remote_profile: Option<PathBuf>,
    #[arg(long, requires = "remote_profile")]
    remote_agent: Option<String>,
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let runtime = if let Some(profile) = args.remote_profile {
        Runtime::remote(&profile, args.remote_agent.as_deref())?
    } else if let Some(context) = args.docker_context {
        Runtime::docker(&context, args.docker_binary.as_deref())?
    } else if args.demo {
        Runtime::demo()?
    } else {
        Runtime::apple_at(args.apple_binary.as_deref())?
    };
    let data = args.state.join(&runtime.storage_id);
    containerhub_remote::store::private_directory(&data)?;
    let _state_lock = containerhub_remote::profile::instance_lock(&data)?;
    let provider = runtime.client.clone();
    let operations = Arc::new(OperationService::open(provider.clone(), data).await?);
    let output = Arc::new(Mutex::new(tokio::io::stdout()));
    let slots = Arc::new(Semaphore::new(32));
    let mut jobs = JoinSet::new();
    let mut input = tokio::io::stdin();
    let mut pending = Zeroizing::new(Vec::with_capacity(MAX_FRAME + 8192));
    let mut chunk = Zeroizing::new([0u8; 8192]);
    loop {
        let n = input.read(&mut chunk[..]).await?;
        if n == 0 {
            break;
        }
        pending.extend_from_slice(&chunk[..n]);
        chunk.zeroize();
        while let Some(end) = pending.iter().position(|b| *b == b'\n') {
            ensure!(end <= MAX_FRAME, "request frame exceeds limit");
            let frame = Zeroizing::new(pending[..=end].to_vec());
            let remaining = Zeroizing::new(pending[end + 1..].to_vec());
            pending.zeroize();
            pending.extend_from_slice(&remaining);
            let request: Envelope = serde_json::from_slice(&frame)
                .map_err(|_| anyhow::anyhow!("invalid request frame"))?;
            ensure!(
                !request.id.is_empty() && request.id.len() <= 128,
                "invalid request ID"
            );
            let id = request.id;
            if request.version != VERSION {
                write(
                    &output,
                    Response::new(
                        id,
                        Err(ManagementError::new(
                            ErrorKind::Unsupported,
                            "不支持该协议版本",
                        )),
                    ),
                )
                .await?;
                continue;
            }
            let Ok(permit) = slots.clone().try_acquire_owned() else {
                write(
                    &output,
                    Response::new(
                        id,
                        Err(ManagementError::new(
                            ErrorKind::Conflict,
                            "并发请求已达 32 项上限",
                        )),
                    ),
                )
                .await?;
                continue;
            };
            let (provider, operations, output) =
                (provider.clone(), operations.clone(), output.clone());
            jobs.spawn(async move {
                let _permit = permit;
                let response =
                    dispatch::dispatch(request.request, provider.as_ref(), &operations).await;
                write(&output, Response::new(id, response)).await
            });
            while let Some(result) = jobs.try_join_next() {
                result??;
            }
        }
        ensure!(pending.len() <= MAX_FRAME, "request frame exceeds limit");
    }
    ensure!(pending.is_empty(), "truncated request frame");
    while let Some(result) = jobs.join_next().await {
        result??;
    }
    operations.shutdown().await;
    Ok(())
}
async fn write(output: &Arc<Mutex<tokio::io::Stdout>>, response: Response) -> Result<()> {
    let mut bytes = serde_json::to_vec(&response)?;
    if bytes.len() > MAX_FRAME {
        bytes = serde_json::to_vec(&Response::new(
            response.id,
            Err(ManagementError::new(
                ErrorKind::InvalidData,
                "响应超过 1 MiB 上限",
            )),
        ))?;
    }
    bytes.push(b'\n');
    let mut output = output.lock().await;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        output.write_all(&bytes).await?;
        output.flush().await
    })
    .await??;
    Ok(())
}
