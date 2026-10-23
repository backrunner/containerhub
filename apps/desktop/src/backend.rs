pub mod environment;
mod image_batch;
mod stats;
mod usage;
use crate::theme::Appearance;
use async_channel::{Receiver, Sender};
use containerhub_core::*;
use containerhub_management::queue::OperationRequest;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::watch;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub appearance: Appearance,
    #[serde(default)]
    pub apple_binary: Option<PathBuf>,
    #[serde(default)]
    pub sort: ContainerSort,
    #[serde(default)]
    pub sort_descending: bool,
}

pub enum Request {
    Refresh,
    Diagnostics,
    ConfigureRuntime(Settings),
    Resources(ResourceKind, u64),
    InspectImage {
        environment: String,
        reference: String,
        generation: u64,
    },
    InspectImageBatch {
        environment: String,
        references: Vec<String>,
        reply: tokio::sync::oneshot::Sender<image_batch::Results>,
    },
    InspectContainer {
        key: ResourceKey,
        generation: u64,
    },
    Run(Box<Operation>, SecretEnvironment),
    Cancel(u64),
    Reconcile(u64),
    SaveSettings(Settings),
}
pub enum Event {
    Environment(Box<environment::Sample>),
    Snapshot(Result<Snapshot, ManagementError>),
    Diagnostics(Result<RuntimeDiagnostics, ManagementError>),
    RuntimeConfigured(Result<Option<PathBuf>, ManagementError>),
    Resources(ResourceKind, u64, Result<Vec<Resource>, ManagementError>),
    Image(u64, Result<ImageDetails, ManagementError>),
    Logs(u64, Result<LogSnapshot, ManagementError>),
    Stats(u64, Result<ContainerStats, ManagementError>),
    Usage(
        u64,
        Vec<(ResourceKey, Result<ContainerStats, ManagementError>)>,
    ),
    Operation(Operation),
    ContainerDetails(
        u64,
        Box<Result<containerhub_core::ContainerDetails, ManagementError>>,
    ),
    ControlError(String),
    StorageError,
    WriterUnavailable,
}
#[derive(Clone)]
pub struct LogRequest {
    pub generation: u64,
    pub key: ResourceKey,
    pub source: LogSource,
    pub automatic: bool,
}
#[derive(Clone)]
pub struct StatsRequest {
    pub generation: u64,
    pub key: ResourceKey,
}
#[derive(Clone)]
pub struct UsageRequest {
    pub generation: u64,
    pub keys: Vec<ResourceKey>,
}
#[derive(Clone)]
pub struct Backend {
    tx: Sender<Request>,
    writes: Sender<OperationRequest>,
    pub logs: watch::Sender<Option<LogRequest>>,
    pub stats: watch::Sender<Option<StatsRequest>>,
    pub usage: watch::Sender<Option<UsageRequest>>,
    pub environment: watch::Sender<Option<environment::Watch>>,
}
impl Backend {
    fn channels() -> (
        Self,
        Receiver<Request>,
        Receiver<OperationRequest>,
        watch::Receiver<Option<LogRequest>>,
    ) {
        let (tx, rx) = async_channel::bounded(32);
        let (writes, write_rx) = async_channel::bounded(32);
        let (logs, log_rx) = watch::channel(None);
        let (stats, _) = watch::channel(None);
        let (usage, _) = watch::channel(None);
        let (environment, _) = watch::channel(None);
        (
            Self {
                tx,
                writes,
                logs,
                stats,
                usage,
                environment,
            },
            rx,
            write_rx,
            log_rx,
        )
    }
    pub fn start(provider: Arc<dyn ManagementClient>, data: PathBuf) -> (Self, Receiver<Event>) {
        let (backend, rx, write_rx, log_rx) = Self::channels();
        let stat_rx = backend.stats.subscribe();
        let usage_rx = backend.usage.subscribe();
        let environment_rx = backend.environment.subscribe();
        let (events, event_rx) = async_channel::bounded(64);
        std::thread::Builder::new()
            .name("containerhub-runtime".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("async runtime");
                runtime.block_on(async move {
                    // Reads remain responsive during slow mutations. The journal serializes acceptance;
                    // execution is bounded to four resources and serial within each resource.
                    tokio::join!(
                        discovery(provider.clone(), &data, rx, events.clone()),
                        operations(provider.clone(), &data, write_rx, events.clone()),
                        stats::worker(provider.clone(), stat_rx, events.clone()),
                        usage::worker(provider.clone(), usage_rx, events.clone()),
                        environment::worker(environment_rx, events.clone()),
                        log_worker(provider, log_rx, events),
                    );
                });
            })
            .expect("runtime thread");
        (backend, event_rx)
    }
    pub fn send(&self, request: Request) -> bool {
        match request {
            Request::Run(op, secrets) => self
                .writes
                .try_send(OperationRequest::Submit(op, secrets))
                .is_ok(),
            Request::Cancel(id) => {
                let (reply, _) = tokio::sync::oneshot::channel();
                self.writes
                    .try_send(OperationRequest::Cancel { id, reply })
                    .is_ok()
            }
            Request::Reconcile(id) => {
                let (reply, _) = tokio::sync::oneshot::channel();
                self.writes
                    .try_send(OperationRequest::Reconcile { id, reply })
                    .is_ok()
            }
            request => self.tx.try_send(request).is_ok(),
        }
    }
    #[cfg(feature = "ui-validation")]
    pub fn isolated() -> (
        Self,
        Receiver<Event>,
        Receiver<Request>,
        Receiver<OperationRequest>,
    ) {
        let (backend, requests, writes, _) = Self::channels();
        let (_, events) = async_channel::bounded(1);
        (backend, events, requests, writes)
    }
}
async fn discovery(
    provider: Arc<dyn ManagementClient>,
    data: &Path,
    rx: Receiver<Request>,
    events: Sender<Event>,
) {
    let mut backoff = 2u64;
    if events
        .send(Event::Snapshot(provider.snapshot().await))
        .await
        .is_err()
    {
        return;
    }
    loop {
        match tokio::time::timeout(Duration::from_secs(backoff), rx.recv()).await {
            Ok(Err(_)) => break,
            Ok(Ok(Request::Diagnostics)) => {
                if events
                    .send(Event::Diagnostics(provider.diagnostics().await))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Ok(Ok(Request::ConfigureRuntime(settings))) => {
                let result = async {
                    if let Some(path) = &settings.apple_binary {
                        containerhub_apple::validate_binary(path)?;
                        let candidate =
                            containerhub_apple::AppleProvider::with_binary(path.clone())?;
                        let report = candidate.diagnostics().await?;
                        if report.cli_version.is_none() {
                            return Err(ManagementError::new(
                                ErrorKind::Unsupported,
                                "所选文件未返回可识别的 Apple container 版本，未保存路径",
                            ));
                        }
                    }
                    save_json(&data.join("settings.json"), &settings).map_err(|_| {
                        ManagementError::new(ErrorKind::Unavailable, "无法保存 CLI 路径")
                    })?;
                    Ok(settings.apple_binary)
                }
                .await;
                let _ = events.send(Event::RuntimeConfigured(result)).await;
            }
            Ok(Ok(Request::SaveSettings(mut settings))) => {
                // Appearance/sort requests can have been queued before a slow CLI probe completes.
                // Preserve the independently confirmed path instead of restoring their stale copy.
                settings.apple_binary = read_json::<Settings>(&data.join("settings.json"))
                    .and_then(|stored| stored.apple_binary);
                if save_json(&data.join("settings.json"), &settings).is_err() {
                    let _ = events.send(Event::StorageError).await;
                }
            }
            Ok(Ok(Request::Resources(kind, generation))) => {
                if events
                    .send(Event::Resources(
                        kind,
                        generation,
                        provider.resources(kind).await,
                    ))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Ok(Ok(Request::InspectImageBatch {
                environment,
                references,
                reply,
            })) => {
                let result = image_batch::inspect(provider.clone(), environment, references).await;
                let _ = reply.send(result);
            }
            Ok(Ok(Request::InspectImage {
                environment,
                reference,
                generation,
            })) => {
                if events
                    .send(Event::Image(
                        generation,
                        provider.inspect_image(&environment, &reference).await,
                    ))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Ok(Ok(Request::InspectContainer { key, generation })) => {
                if events
                    .send(Event::ContainerDetails(
                        generation,
                        Box::new(provider.inspect_container(&key).await),
                    ))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Ok(Ok(Request::Refresh)) | Err(_) => {
                let result = provider.snapshot().await;
                backoff = if result.is_err() {
                    (backoff * 2).min(30)
                } else {
                    2
                };
                if events.send(Event::Snapshot(result)).await.is_err() {
                    break;
                }
            }
            Ok(Ok(Request::Run(..) | Request::Cancel(_) | Request::Reconcile(_))) => {
                unreachable!("writes use a separate channel")
            }
        }
    }
}
async fn operations(
    provider: Arc<dyn ManagementClient>,
    data: &Path,
    rx: Receiver<OperationRequest>,
    events: Sender<Event>,
) {
    let (tx, updates) = async_channel::bounded(64);
    let forward = async {
        while let Ok(update) = updates.recv().await {
            use containerhub_management::OperationEvent as E;
            let event = match update {
                E::Ready => continue,
                E::ControlError(message) => Event::ControlError(message),
                E::Operation(op) => Event::Operation(*op),
                E::StorageError => Event::StorageError,
                E::WriterUnavailable => Event::WriterUnavailable,
            };
            if events.send(event).await.is_err() {
                break;
            }
        }
    };
    tokio::join!(
        containerhub_management::queue::operations(provider, data, rx, tx),
        forward
    );
}
async fn log_worker(
    provider: Arc<dyn ManagementClient>,
    mut rx: watch::Receiver<Option<LogRequest>>,
    events: Sender<Event>,
) {
    loop {
        let request = rx.borrow_and_update().clone();
        let Some(request) = request else {
            if rx.changed().await.is_err() {
                break;
            }
            continue;
        };
        // Selection changes cancel the pending read (and its kill-on-drop CLI process).
        let result = tokio::select! {
            biased;
            changed = rx.changed() => { if changed.is_err() { break; } continue; }
            result = provider.logs(&request.key, request.source) => result,
        };
        let failed = result.is_err();
        if events
            .send(Event::Logs(request.generation, result))
            .await
            .is_err()
        {
            break;
        }
        if request.automatic {
            tokio::select! {
                changed = rx.changed() => { if changed.is_err() { break; } }
                _ = tokio::time::sleep(Duration::from_secs(if failed { 10 } else { 2 })) => {}
            }
        } else if rx.changed().await.is_err() {
            break;
        }
    }
}
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}
use containerhub_management::storage::save_json;

#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod tests;
