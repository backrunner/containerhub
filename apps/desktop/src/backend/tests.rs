use super::*;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};

struct SlowProvider {
    fake: containerhub_core::fake::FakeProvider,
    calls: AtomicUsize,
    entered: Sender<String>,
}
#[async_trait]
impl ManagementClient for SlowProvider {
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        self.fake.snapshot().await
    }
    async fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        self.fake.resources(kind).await
    }
    async fn logs(
        &self,
        key: &ResourceKey,
        source: LogSource,
    ) -> Result<LogSnapshot, ManagementError> {
        self.entered.send(key.id.clone()).await.unwrap();
        if key.id == "slow" {
            std::future::pending::<()>().await;
        }
        Ok(LogSnapshot::from_bytes(key.clone(), source, b"ready\n"))
    }
    async fn lifecycle(&self, _: &ResourceKey, _: LifecycleAction) -> Result<(), ManagementError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.send("write".into()).await.unwrap();
        std::future::pending().await
    }
}
fn provider() -> (Arc<SlowProvider>, Receiver<String>) {
    let (entered, rx) = async_channel::bounded(8);
    (
        Arc::new(SlowProvider {
            fake: Default::default(),
            calls: AtomicUsize::new(0),
            entered,
        }),
        rx,
    )
}
fn key(id: &str) -> ResourceKey {
    ResourceKey {
        provider: containerhub_core::ProviderKind::Apple,
        namespace: None,
        environment: "test".into(),
        id: id.into(),
        incarnation: "1".into(),
    }
}
fn operation() -> Operation {
    Operation {
        receipt: None,
        id: 1,
        command: Command::Lifecycle {
            key: key("slow"),
            action: LifecycleAction::Start,
        },
        result: None,
        progress: None,
        cancel_requested: false,
        state: OperationState::Queued,
        started_at: now(),
        finished_at: None,
        message: None,
    }
}
async fn next<T>(rx: &Receiver<T>) -> T {
    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("worker responded")
        .unwrap()
}
fn directory(name: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("containerhub-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}
#[tokio::test]
async fn discovery_remains_responsive_while_write_is_waiting() {
    let (provider, entered) = provider();
    let (events, rx) = async_channel::bounded(16);
    let (commands, commands_rx) = async_channel::bounded(8);
    let (writes, writes_rx) = async_channel::bounded(8);
    let data = directory("responsive");
    let reader = tokio::spawn({
        let p = provider.clone();
        let data = data.clone();
        let e = events.clone();
        async move { discovery(p, &data, commands_rx, e).await }
    });
    let writer = tokio::spawn({
        let p = provider.clone();
        let data = data.clone();
        async move { operations(p, &data, writes_rx, events).await }
    });
    assert!(matches!(next(&rx).await, Event::Snapshot(Ok(_))));
    writes.send(operation().into()).await.unwrap();
    assert_eq!(next(&entered).await, "write");
    assert!(matches!(
        next(&rx).await,
        Event::Operation(Operation {
            state: OperationState::Queued,
            ..
        })
    ));
    assert!(matches!(
        next(&rx).await,
        Event::Operation(Operation {
            state: OperationState::Running,
            ..
        })
    ));
    commands.send(Request::Refresh).await.unwrap();
    assert!(matches!(next(&rx).await, Event::Snapshot(Ok(_))));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let saved: Vec<Operation> = read_json(&data.join("operations.json")).unwrap();
    assert_eq!(saved[0].state, OperationState::Running);
    reader.abort();
    writer.abort();
    let _ = tokio::join!(reader, writer);
    std::fs::remove_dir_all(data).unwrap();
}
#[tokio::test]
async fn selection_change_cancels_a_slow_log_read() {
    let (provider, entered) = provider();
    let (events, rx) = async_channel::bounded(8);
    let (requests, requests_rx) = watch::channel(Some(LogRequest {
        generation: 1,
        key: key("slow"),
        source: LogSource::Standard,
        automatic: true,
    }));
    let task = tokio::spawn(log_worker(provider, requests_rx, events));
    assert_eq!(next(&entered).await, "slow");
    requests.send_replace(Some(LogRequest {
        generation: 2,
        key: key("new"),
        source: LogSource::Boot,
        automatic: false,
    }));
    assert_eq!(next(&entered).await, "new");
    match next(&rx).await {
        Event::Logs(2, Ok(log)) => {
            assert_eq!(log.key.id, "new");
            assert_eq!(log.source, LogSource::Boot);
        }
        _ => panic!("unexpected stale log result"),
    }
    requests.send_replace(None);
    drop(requests);
    task.await.unwrap();
    assert!(rx.try_recv().is_err());
}
#[tokio::test]
async fn a_journal_failure_prevents_the_lifecycle_command() {
    let (provider, _) = provider();
    let (events, rx) = async_channel::bounded(8);
    let (writes, writes_rx) = async_channel::bounded(1);
    let data = directory("journal-failure");
    std::fs::create_dir_all(data.join("operations.json.tmp")).unwrap();
    writes.send(operation().into()).await.unwrap();
    drop(writes);
    operations(provider.clone(), &data, writes_rx, events).await;
    assert!(matches!(
        next(&rx).await,
        Event::Operation(Operation {
            state: OperationState::Failed,
            ..
        })
    ));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    std::fs::remove_dir_all(data).unwrap();
}
struct GatedProvider {
    gate: tokio::sync::Semaphore,
    entered: Sender<String>,
    active: AtomicUsize,
    maximum: AtomicUsize,
    keys: std::sync::Mutex<std::collections::HashSet<ResourceKey>>,
}
#[async_trait]
impl ManagementClient for GatedProvider {
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        containerhub_core::fake::FakeProvider::default()
            .snapshot()
            .await
    }
    async fn resources(&self, _: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        Ok(vec![])
    }
    async fn logs(
        &self,
        key: &ResourceKey,
        source: LogSource,
    ) -> Result<LogSnapshot, ManagementError> {
        Ok(LogSnapshot::from_bytes(key.clone(), source, b""))
    }
    async fn lifecycle(
        &self,
        key: &ResourceKey,
        _: LifecycleAction,
    ) -> Result<(), ManagementError> {
        assert!(
            self.keys.lock().unwrap().insert(key.clone()),
            "same resource executed concurrently"
        );
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        self.entered.send(key.id.clone()).await.unwrap();
        self.gate.acquire().await.unwrap().forget();
        self.keys.lock().unwrap().remove(key);
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
}
#[tokio::test]
async fn queued_intents_are_durable_while_four_resources_run_and_each_resource_is_serial() {
    let (entered, entered_rx) = async_channel::bounded(16);
    let provider = Arc::new(GatedProvider {
        gate: tokio::sync::Semaphore::new(0),
        entered,
        active: AtomicUsize::new(0),
        maximum: AtomicUsize::new(0),
        keys: Default::default(),
    });
    let (events, rx) = async_channel::bounded(64);
    let (writes, writes_rx) = async_channel::bounded(16);
    let data = directory("durable-queue");
    let task = tokio::spawn({
        let provider = provider.clone();
        let data = data.clone();
        async move { operations(provider, &data, writes_rx, events).await }
    });
    for (index, id) in ["a", "a", "b", "c", "d", "e"].into_iter().enumerate() {
        let mut op = operation();
        op.id = index as u64 + 1;
        op.command = Command::Lifecycle {
            key: key(id),
            action: LifecycleAction::Start,
        };
        writes.send(op.into()).await.unwrap();
    }
    drop(writes);
    loop {
        if matches!(
            next(&rx).await,
            Event::Operation(Operation {
                id: 6,
                state: OperationState::Queued,
                ..
            })
        ) {
            break;
        }
    }
    for _ in 0..4 {
        next(&entered_rx).await;
    }
    let history: Vec<Operation> = read_json(&data.join("operations.json")).unwrap();
    assert_eq!(
        history.iter().find(|op| op.id == 2).unwrap().state,
        OperationState::Queued
    );
    assert_eq!(
        history.iter().find(|op| op.id == 6).unwrap().state,
        OperationState::Queued
    );
    assert_eq!(provider.maximum.load(Ordering::SeqCst), 4);
    provider.gate.add_permits(6);
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    let history: Vec<Operation> = read_json(&data.join("operations.json")).unwrap();
    assert!(
        history
            .iter()
            .all(|op| op.state == OperationState::Succeeded)
    );
    assert_eq!(provider.maximum.load(Ordering::SeqCst), 4);
    std::fs::remove_dir_all(data).unwrap();
}
#[tokio::test]
async fn crash_recovery_reconciles_observed_state_without_replaying_queued_or_restart_commands() {
    let (provider, _) = provider();
    let snapshot = provider.snapshot().await.unwrap();
    let key = snapshot
        .containers
        .iter()
        .find(|c| c.state == ContainerState::Running)
        .unwrap()
        .key
        .clone();
    let data = directory("recovery");
    let mut running = operation();
    running.command = Command::Lifecycle {
        key: key.clone(),
        action: LifecycleAction::Start,
    };
    running.state = OperationState::Running;
    let mut queued = running.clone();
    queued.id = 2;
    queued.state = OperationState::Queued;
    let mut restart = running.clone();
    restart.id = 3;
    restart.command = Command::Lifecycle {
        key,
        action: LifecycleAction::Restart,
    };
    save_json(
        &data.join("operations.json"),
        &vec![running, queued, restart],
    )
    .unwrap();
    let (events, rx) = async_channel::bounded(8);
    let (writes, writes_rx) = async_channel::bounded(1);
    drop(writes);
    operations(provider.clone(), &data, writes_rx, events).await;
    assert!(matches!(
        next(&rx).await,
        Event::Operation(Operation {
            id: 1,
            state: OperationState::Succeeded,
            ..
        })
    ));
    assert!(matches!(
        next(&rx).await,
        Event::Operation(Operation {
            id: 2,
            state: OperationState::Failed,
            ..
        })
    ));
    assert!(matches!(
        next(&rx).await,
        Event::Operation(Operation {
            id: 3,
            state: OperationState::UnknownOutcome,
            ..
        })
    ));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    std::fs::remove_dir_all(data).unwrap();
}
#[tokio::test]
async fn corrupt_journal_stops_writes_without_overwriting_evidence() {
    let (provider, _) = provider();
    let data = directory("corrupt-journal");
    std::fs::write(data.join("operations.json"), b"broken journal").unwrap();
    let (events, rx) = async_channel::bounded(8);
    let (writes, writes_rx) = async_channel::bounded(1);
    writes.send(operation().into()).await.unwrap();
    drop(writes);
    operations(provider.clone(), &data, writes_rx, events).await;
    assert!(matches!(next(&rx).await, Event::WriterUnavailable));
    assert_eq!(
        std::fs::read(data.join("operations.json")).unwrap(),
        b"broken journal"
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    std::fs::remove_dir_all(data).unwrap();
}
