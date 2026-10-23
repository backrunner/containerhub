use super::*;
use service::OperationService;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Provider {
    directory: std::path::PathBuf,
    submitted: AtomicUsize,
    reads: AtomicUsize,
    break_save: bool,
}
#[async_trait::async_trait]
impl ManagementClient for Provider {
    fn prepare_execution(&self, _: &Command) -> Result<Option<ExecutionReceipt>, ManagementError> {
        if self.break_save {
            std::fs::create_dir(self.directory.join("operations.json.tmp")).unwrap();
        }
        Ok(Some(receipt()))
    }
    async fn execute_prepared(
        &self,
        _: &Command,
        receipt: Option<&ExecutionReceipt>,
        _: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        self.submitted.fetch_add(1, Ordering::SeqCst);
        let history: Vec<Operation> =
            serde_json::from_slice(&std::fs::read(self.directory.join("operations.json")).unwrap())
                .unwrap();
        assert_eq!(history[0].receipt.as_ref(), receipt);
        assert_eq!(history[0].state, OperationState::Running);
        Err(ManagementError::new(ErrorKind::UnknownOutcome, "ACK lost"))
    }
    async fn resume_execution(
        &self,
        _: &Command,
        _: &ExecutionReceipt,
        _: &ExecutionControl,
    ) -> Result<RecoveredExecution, ManagementError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(RecoveredExecution {
            state: OperationState::Succeeded,
            result: Some(CommandResult::Confirmed),
            error: None,
            cancel_requested: false,
        })
    }
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        fake::FakeProvider::default().snapshot().await
    }
    async fn resources(&self, _: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        unreachable!()
    }
    async fn logs(&self, _: &ResourceKey, _: LogSource) -> Result<LogSnapshot, ManagementError> {
        unreachable!()
    }
    async fn lifecycle(&self, _: &ResourceKey, _: LifecycleAction) -> Result<(), ManagementError> {
        unreachable!()
    }
}
fn receipt() -> ExecutionReceipt {
    ExecutionReceipt::Remote {
        agent: "test-agent".into(),
        owner: "test-owner".into(),
        request_id: "test-request-123456".into(),
        expires_at: 1,
    }
}
fn pending() -> Operation {
    Operation {
        id: 1,
        command: Command::PullImage {
            environment: "test".into(),
            reference: "example.invalid/image:v1".into(),
        },
        receipt: Some(receipt()),
        state: OperationState::UnknownOutcome,
        result: None,
        progress: None,
        cancel_requested: false,
        started_at: 1,
        finished_at: None,
        message: None,
    }
}
async fn terminal(service: &OperationService) -> Operation {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
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
async fn association_is_persisted_before_send_and_persistence_failure_prevents_submission() {
    for break_save in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let provider = Arc::new(Provider {
            directory: dir.path().to_owned(),
            submitted: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
            break_save,
        });
        let service = OperationService::open(provider.clone(), dir.path().to_owned())
            .await
            .unwrap();
        service.submit(1, pending().command).await.unwrap();
        let op = terminal(&service).await;
        assert_eq!(
            provider.submitted.load(Ordering::SeqCst),
            usize::from(!break_save)
        );
        assert_eq!(
            op.state,
            if break_save {
                OperationState::Failed
            } else {
                OperationState::UnknownOutcome
            }
        );
        if !break_save {
            service.reconcile(1).await.unwrap();
            assert_eq!(terminal(&service).await.state, OperationState::Succeeded);
            assert_eq!(provider.reads.load(Ordering::SeqCst), 1);
            assert_eq!(provider.submitted.load(Ordering::SeqCst), 1);
        }
        service.shutdown().await;
    }
}
#[tokio::test]
async fn startup_recovery_is_bounded_and_queued_commands_never_replay() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider {
        directory: dir.path().to_owned(),
        submitted: AtomicUsize::new(0),
        reads: AtomicUsize::new(0),
        break_save: false,
    };
    let mut history: Vec<_> = (1..=35)
        .map(|id| {
            let mut op = pending();
            op.id = id;
            op
        })
        .collect();
    history[34].state = OperationState::Queued;
    let queue = recovery::restore(&provider, &mut history).await;
    assert_eq!(queue.len(), 32);
    assert_eq!(history[32].state, OperationState::UnknownOutcome);
    assert_eq!(history[34].state, OperationState::Failed);
    assert_eq!(
        provider.reads.load(Ordering::SeqCst),
        0,
        "network reads happen after Ready"
    );
    assert_eq!(provider.submitted.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn rebuild_recovery_checks_replacement_ports_and_original_running_state() {
    let mut snapshot = fake::FakeProvider::default().snapshot().await.unwrap();
    let original = snapshot.containers[0].clone();
    let mut replacement = original.clone();
    replacement.key.incarnation.push_str("-rebuilt");
    replacement.ports = vec![Port {
        host: "127.0.0.1".into(),
        host_port: 18080,
        container_port: 80,
        protocol: "tcp".into(),
    }];
    for desired in [ContainerState::Running, ContainerState::Stopped] {
        for observed in [ContainerState::Running, ContainerState::Stopped] {
            let mut op = pending();
            op.receipt = None;
            op.state = OperationState::Running;
            op.command = Command::RebuildPorts {
                key: original.key.clone(),
                expected: original.ports.clone(),
                expected_state: desired.clone(),
                ports: replacement.ports.clone(),
            };
            op.result = Some(CommandResult::Container(Box::new(replacement.clone())));
            snapshot.containers[0] = replacement.clone();
            snapshot.containers[0].state = observed.clone();
            assert!(recovery::created(&mut op, Some(&snapshot)));
            assert_eq!(
                op.state,
                if desired == observed {
                    OperationState::Succeeded
                } else if observed == ContainerState::Stopped {
                    OperationState::Failed
                } else {
                    OperationState::UnknownOutcome
                }
            );
            snapshot.containers[0].ports[0].host_port = 18081;
            assert!(recovery::created(&mut op, Some(&snapshot)));
            assert_eq!(op.state, OperationState::UnknownOutcome);
            assert!(start_created_command(&op.command, op.result.as_ref(), &snapshot).is_none());
        }
    }
}
