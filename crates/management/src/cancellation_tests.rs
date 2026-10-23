use crate::service::OperationService;
use containerhub_core::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

struct Slow {
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl ManagementClient for Slow {
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        containerhub_core::fake::FakeProvider::default()
            .snapshot()
            .await
    }
    async fn resources(&self, _: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        Ok(vec![])
    }
    async fn lifecycle(&self, _: &ResourceKey, _: LifecycleAction) -> Result<(), ManagementError> {
        unreachable!()
    }
    async fn logs(&self, _: &ResourceKey, _: LogSource) -> Result<LogSnapshot, ManagementError> {
        unreachable!()
    }
    async fn execute_controlled(
        &self,
        _: &Command,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        control.check_before_start()?;
        self.calls.fetch_add(1, Ordering::SeqCst);
        control.report(OperationProgress::Fetching);
        while !control.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Err(ManagementError::new(
            ErrorKind::UnknownOutcome,
            "runtime cancellation requires reconciliation",
        ))
    }
}
#[tokio::test]
async fn cancellation_is_durable_scoped_idempotent_and_never_replays() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Arc::new(Slow {
        calls: AtomicUsize::new(0),
    });
    let service = OperationService::open(provider.clone(), directory.path().to_owned())
        .await
        .unwrap();
    let command = Command::PullImage {
        environment: "demo".into(),
        reference: "example.invalid/test:v1".into(),
    };
    service.submit(1, command.clone()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while service.get(1).unwrap().progress != Some(OperationProgress::Fetching) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    service.submit(2, command.clone()).await.unwrap();
    assert_eq!(service.get(2).unwrap().state, OperationState::Queued);
    assert_eq!(
        service.cancel(2).await.unwrap().state,
        OperationState::Cancelled
    );
    assert_eq!(
        service.cancel(2).await.unwrap().state,
        OperationState::Cancelled
    );
    assert_eq!(
        service.cancel(999).await.unwrap_err().kind,
        ErrorKind::NotFound
    );
    let journal = directory.path().join("operations.json");
    let backup = directory.path().join("operations.saved");
    std::fs::rename(&journal, &backup).unwrap();
    std::fs::create_dir(&journal).unwrap();
    assert_eq!(
        service.cancel(1).await.unwrap_err().kind,
        ErrorKind::Unavailable
    );
    assert!(
        !service.get(1).unwrap().cancel_requested,
        "failed persistence must not signal cancellation"
    );
    assert_eq!(service.get(1).unwrap().state, OperationState::Running);
    std::fs::remove_dir(&journal).unwrap();
    std::fs::rename(&backup, &journal).unwrap();
    service.cancel(1).await.unwrap();
    service.shutdown().await;
    assert_eq!(
        service.get(1).unwrap().state,
        OperationState::UnknownOutcome
    );
    assert!(service.get(1).unwrap().cancel_requested);
    let restored = OperationService::open(provider.clone(), directory.path().to_owned())
        .await
        .unwrap();
    assert_eq!(
        restored.submit(2, command).await.unwrap().state,
        OperationState::Cancelled
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    restored.shutdown().await;
}
