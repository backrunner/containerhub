use super::*;
use service::OperationService;

struct FlakyCheckpoint(AtomicUsize);
#[async_trait::async_trait]
impl CreationCheckpoint for FlakyCheckpoint {
    async fn save(&self, _: &Container) -> Result<(), ManagementError> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(ManagementError::new(
                ErrorKind::Unavailable,
                "disk unavailable",
            ))
        } else {
            Ok(())
        }
    }
}
#[tokio::test]
async fn failed_checkpoint_is_never_mistaken_for_a_durable_ack() {
    let checkpoint = Arc::new(FlakyCheckpoint(AtomicUsize::new(0)));
    let control =
        ExecutionControl::with_checkpoint(SecretEnvironment::default(), checkpoint.clone());
    let container = fake::FakeProvider::default()
        .snapshot()
        .await
        .unwrap()
        .containers
        .remove(0);
    assert!(control.checkpoint_created(&container).await.is_err());
    assert!(control.created().is_some());
    control.checkpoint_created(&container).await.unwrap();
    control.checkpoint_created(&container).await.unwrap();
    assert_eq!(checkpoint.0.load(Ordering::SeqCst), 2);
}
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Provider {
    inner: fake::FakeProvider,
    path: PathBuf,
    mode: u8,
    creates: AtomicUsize,
    starts: AtomicUsize,
    entered: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl ManagementClient for Provider {
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        let mut snapshot = self.inner.snapshot().await?;
        if self.mode == 5 {
            snapshot.lifecycle_supported = false;
        }
        Ok(snapshot)
    }
    async fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        self.inner.resources(kind).await
    }
    async fn logs(
        &self,
        key: &ResourceKey,
        source: LogSource,
    ) -> Result<LogSnapshot, ManagementError> {
        self.inner.logs(key, source).await
    }
    async fn execute_controlled(
        &self,
        command: &Command,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        assert!(!command.starts_after_create());
        if matches!(command, Command::CreateContainer { .. }) {
            self.creates.fetch_add(1, Ordering::SeqCst);
        }
        let result = self.inner.execute_controlled(command, control).await?;
        if self.mode == 4 {
            std::fs::create_dir(self.path.join("operations.json.tmp")).unwrap();
        }
        Ok(result)
    }
    async fn lifecycle(
        &self,
        key: &ResourceKey,
        action: LifecycleAction,
    ) -> Result<(), ManagementError> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        let history: Vec<Operation> =
            serde_json::from_slice(&std::fs::read(self.path.join("operations.json")).unwrap())
                .unwrap();
        assert_eq!(history[0].state, OperationState::Running);
        let Some(CommandResult::Container(created)) = &history[0].result else {
            panic!("identity must be durable before start")
        };
        assert_eq!(&created.key, key);
        match self.mode {
            1 => {
                return Err(ManagementError::new(
                    ErrorKind::Unavailable,
                    "start rejected",
                ));
            }
            2 | 3 => {
                if self.mode == 3 {
                    self.inner.lifecycle(key, action).await?;
                }
                self.entered.notify_one();
                std::future::pending::<()>().await;
            }
            _ => {}
        }
        self.inner.lifecycle(key, action).await
    }
}
async fn fixture(mode: u8) -> (tempfile::TempDir, Arc<Provider>, Command) {
    let directory = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider {
        inner: fake::FakeProvider::default(),
        path: directory.path().into(),
        mode,
        creates: AtomicUsize::new(0),
        starts: AtomicUsize::new(0),
        entered: Default::default(),
    });
    let command = Command::CreateContainer {
        environment: "demo".into(),
        spec: CreateContainer {
            name: "workflow-test".into(),
            image: provider.inner.snapshot().await.unwrap().containers[0]
                .image
                .clone(),
            cpus: 1,
            memory_mib: 256,
            ports: vec![],
            options: CreateOptions {
                start_after_create: true,
                ..Default::default()
            },
        },
    };
    (directory, provider, command)
}
async fn done(service: &OperationService) -> Operation {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let op = service.get(1).unwrap();
            if !op.state.is_active() {
                return op;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn create_start_persists_identity_and_preserves_partial_results_without_recreation() {
    for mode in [0, 1, 4, 5] {
        let (directory, provider, command) = fixture(mode).await;
        let service = OperationService::open(provider.clone(), directory.path().into())
            .await
            .unwrap();
        service.submit(1, command.clone()).await.unwrap();
        let op = done(&service).await;
        assert_eq!(
            provider.creates.load(Ordering::SeqCst),
            usize::from(mode != 5)
        );
        assert_eq!(
            provider.starts.load(Ordering::SeqCst),
            usize::from(mode <= 1)
        );
        if mode == 0 {
            assert_eq!(op.state, OperationState::Succeeded);
        }
        if mode == 1 {
            assert_eq!(op.state, OperationState::Failed);
            let snapshot = provider.snapshot().await.unwrap();
            let retry = start_created_command(&command, op.result.as_ref(), &snapshot).unwrap();
            assert!(matches!(
                retry,
                Command::Lifecycle {
                    action: LifecycleAction::Start,
                    ..
                }
            ));
            let key = retry.resource_key().unwrap();
            provider
                .inner
                .execute(&Command::DeleteContainer { key: key.clone() })
                .await
                .unwrap();
            let mut create = command.clone();
            if let Command::CreateContainer { spec, .. } = &mut create {
                spec.options.start_after_create = false;
            }
            provider.inner.execute(&create).await.unwrap();
            assert!(
                start_created_command(
                    &command,
                    op.result.as_ref(),
                    &provider.snapshot().await.unwrap()
                )
                .is_none()
            );
            assert!(
                provider.inner.execute(&retry).await.is_err(),
                "same name replacement must not start"
            );
        }
        if mode == 4 {
            assert!(
                op.result.is_some(),
                "failed journal still exposes confirmed creation in memory"
            );
            std::fs::remove_dir(directory.path().join("operations.json.tmp")).unwrap();
        }
        if mode == 5 {
            assert_eq!(op.state, OperationState::Failed);
            assert!(op.result.is_none());
        }
        // Repeated submission with the original ID returns the result, never another create.
        if mode != 4 {
            service.submit(1, command).await.unwrap();
        }
        service.shutdown().await;
    }
}

#[tokio::test]
async fn crash_after_creation_recovers_only_the_saved_incarnation_without_replaying_either_phase() {
    for mode in [2, 3] {
        let (directory, provider, command) = fixture(mode).await;
        let (tx, requests) = async_channel::bounded(8);
        let (events, updates) = async_channel::bounded(64);
        let runtime = provider.clone();
        let path = directory.path().to_owned();
        let worker =
            tokio::spawn(async move { queue::operations(runtime, &path, requests, events).await });
        assert!(matches!(
            updates.recv().await.unwrap(),
            OperationEvent::Ready
        ));
        tx.send(
            Operation {
                id: 1,
                command,
                receipt: None,
                result: None,
                progress: None,
                cancel_requested: false,
                state: OperationState::Submitting,
                started_at: now(),
                finished_at: None,
                message: None,
            }
            .into(),
        )
        .await
        .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            provider.entered.notified(),
        )
        .await
        .unwrap();
        worker.abort();
        assert!(worker.await.unwrap_err().is_cancelled());
        drop(tx);
        let service = OperationService::open(provider.clone(), directory.path().into())
            .await
            .unwrap();
        let result = service.get(1).unwrap();
        assert_eq!(
            result.state,
            if mode == 2 {
                OperationState::Failed
            } else {
                OperationState::Succeeded
            }
        );
        assert!(result.result.is_some());
        assert_eq!(provider.creates.load(Ordering::SeqCst), 1);
        assert_eq!(provider.starts.load(Ordering::SeqCst), 1);
        service.shutdown().await;
    }
}
