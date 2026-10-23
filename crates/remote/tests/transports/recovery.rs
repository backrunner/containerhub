use super::*;
use containerhub_management::{
    OperationEvent, queue, service::OperationService, storage::save_json,
};
use std::sync::atomic::Ordering;
#[path = "creation.rs"]
mod creation;

struct Fixture {
    _directory: tempfile::TempDir,
    agent: Arc<Agent>,
    store: Arc<BindingStore>,
    provider: Arc<ControlledProvider>,
    client: Arc<RemoteClient>,
    direct: Endpoint,
    owner: SecretKey,
    issuer: SecretKey,
    agent_key: SecretKey,
    observed: tokio::sync::mpsc::UnboundedReceiver<ResourceKey>,
    tasks: Vec<tokio::task::JoinHandle<anyhow::Result<()>>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
impl Fixture {
    async fn new(relayed: bool) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let key = SecretKey::generate();
        let issuer = SecretKey::generate();
        let owner = SecretKey::generate();
        let pin = key.public();
        let store = Arc::new(
            BindingStore::initialize(
                directory.path(),
                Installation {
                    version: VERSION,
                    agent: pin.clone(),
                    app_issuer: issuer.public(),
                    audience: "test".into(),
                },
            )
            .unwrap(),
        );
        let (entered, observed) = tokio::sync::mpsc::unbounded_channel();
        let provider = Arc::new(ControlledProvider {
            inner: fake::FakeProvider::default(),
            release: tokio::sync::Semaphore::new(0),
            entered,
            active: Default::default(),
            calls: Default::default(),
        });
        let agent_key = SecretKey::from_bytes(&key.secret_bytes()).unwrap();
        let agent = Agent::new(store.clone(), key, provider.clone(), "demo".into()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let direct = Endpoint::Direct {
            url: format!("ws://{}", listener.local_addr().unwrap()),
        };
        let mut tasks = vec![tokio::spawn(agent.clone().serve(listener))];
        let credentials = Arc::new(credentials(
            &owner,
            SecretKey::generate(),
            &issuer,
            None,
            vec![
                Permission::Read,
                Permission::Lifecycle,
                Permission::ManageImages,
                Permission::ManageContainers,
                Permission::ManageTrust,
            ],
        ));
        authenticate(&direct, &credentials, None, true, true)
            .await
            .unwrap();
        let endpoint = if relayed {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("ws://{}", listener.local_addr().unwrap());
            tasks.push(tokio::spawn(relay::serve(listener)));
            for _ in 0..4 {
                tasks.push(tokio::spawn(relay::outbound(
                    agent.clone(),
                    url.clone(),
                    true,
                )));
            }
            Endpoint::Relay {
                url,
                route: pin.id(),
            }
        } else {
            direct.clone()
        };
        let client = Arc::new(RemoteClient {
            endpoint,
            credentials,
            pin,
            development: true,
        });
        for _ in 0..30 {
            if client.snapshot().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Self {
            _directory: directory,
            agent,
            store,
            provider,
            client,
            direct,
            owner,
            issuer,
            agent_key,
            observed,
            tasks,
        }
    }
    async fn command(&self) -> Command {
        Command::Lifecycle {
            key: self.client.snapshot().await.unwrap().containers[0]
                .key
                .clone(),
            action: LifecycleAction::Stop,
        }
    }
}
fn operation(command: Command) -> Operation {
    Operation {
        id: 301,
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
}

#[tokio::test]
async fn secret_creation_survives_controller_loss_without_replaying_or_journaling_values() {
    for relay in [false, true] {
        let mut fixture = Fixture::new(relay).await;
        let local = tempfile::tempdir().unwrap();
        let image = fixture.client.snapshot().await.unwrap().containers[0]
            .image
            .clone();
        let command = Command::CreateContainer {
            environment: "demo".into(),
            spec: CreateContainer {
                name: "secret-container".into(),
                image,
                cpus: 1,
                memory_mib: 256,
                ports: vec![],
                options: CreateOptions {
                    start_after_create: true,
                    secret_environment: vec!["API_TOKEN".into()],
                    startup: Some(Box::new(StartupCommand {
                        entrypoint: Some("/custom app".into()),
                        arguments: vec!["two words".into(), "".into(), "--name".into()],
                    })),
                    ..Default::default()
                },
            },
        };
        let mut input = SecretEnvironment::default();
        input
            .insert("API_TOKEN".into(), "remote-injection-canary".into())
            .unwrap();
        let (writes, requests) = async_channel::bounded(8);
        let (events, updates) = async_channel::bounded(64);
        let path = local.path().to_owned();
        let client = fixture.client.clone();
        let worker =
            tokio::spawn(async move { queue::operations(client, &path, requests, events).await });
        assert!(matches!(
            updates.recv().await.unwrap(),
            OperationEvent::Ready
        ));
        writes
            .send(queue::OperationRequest::Submit(
                Box::new(operation(command.clone())),
                input.clone(),
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), fixture.observed.recv())
            .await
            .unwrap()
            .unwrap();
        let bytes = std::fs::read(local.path().join("operations.json")).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("remote-injection-canary"));
        let records: Vec<Operation> = serde_json::from_slice(&bytes).unwrap();
        let receipt = records[0].receipt.clone().unwrap();
        let ExecutionReceipt::Remote {
            request_id,
            expires_at,
            ..
        } = &receipt;
        let request = Mutation {
            request_id: request_id.clone(),
            command: command.clone(),
            expires_at: *expires_at,
        };
        // A racing second secret submission is rejected even if its public command matches.
        let (mut socket, _) = authenticate(
            &fixture.client.endpoint,
            &fixture.client.credentials,
            Some(&fixture.client.pin),
            false,
            true,
        )
        .await
        .unwrap();
        socket
            .send(&Request::SubmitWithSecrets {
                request: request.clone(),
                secrets: input,
            })
            .await
            .unwrap();
        let Response::Error(error) = socket.receive().await.unwrap() else {
            panic!("duplicate accepted")
        };
        assert_eq!(error.kind, ErrorKind::Conflict);
        worker.abort();
        assert!(worker.await.unwrap_err().is_cancelled());
        drop(writes);
        let restored = OperationService::open(fixture.client.clone(), local.path().to_owned())
            .await
            .unwrap();
        fixture.provider.release.add_permits(1);
        tokio::time::timeout(Duration::from_secs(5), fixture.observed.recv())
            .await
            .unwrap()
            .unwrap();
        let (_, json) = fixture
            .store
            .operation(request_id, &fixture.owner.public().id())
            .unwrap()
            .unwrap();
        let checkpoint: RemoteOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(checkpoint.state, OperationState::Running);
        assert!(
            matches!(&checkpoint.result, Some(CommandResult::Container(container)) if container.key.id == "secret-container")
        );
        fixture.provider.release.add_permits(1);
        let result = done(&restored, 301).await;
        assert_eq!(result.state, OperationState::Succeeded);
        assert_eq!(result.command, command);
        assert_eq!(result.receipt, Some(receipt));
        assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 2);
        assert!(
            matches!(&result.result, Some(CommandResult::Container(container)) if container.state == ContainerState::Running)
        );
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("remote-injection-canary")
        );
        assert!(
            fixture
                .store
                .operation_records()
                .unwrap()
                .iter()
                .all(|(_, record)| !record.contains("remote-injection-canary"))
        );
        restored.shutdown().await;
        for entry in std::fs::read_dir(fixture._directory.path()).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                assert!(
                    !std::fs::read(path)
                        .unwrap()
                        .windows(b"remote-injection-canary".len())
                        .any(|v| v == b"remote-injection-canary")
                );
            }
        }
    }
}
async fn done(service: &OperationService, id: u64) -> Operation {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let op = service.get(id).unwrap();
            if !op.state.is_active() {
                return op;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn client_crash_recovers_original_agent_request_on_direct_and_relay_without_reexecution() {
    for relay in [false, true] {
        let mut fixture = Fixture::new(relay).await;
        let local = tempfile::tempdir().unwrap();
        let command = fixture.command().await;
        let (writes, requests) = async_channel::bounded(8);
        let (events, updates) = async_channel::bounded(64);
        // Persist on direct; the same association can resume through the relay.
        let client = Arc::new(RemoteClient {
            endpoint: fixture.direct.clone(),
            credentials: fixture.client.credentials.clone(),
            pin: fixture.client.pin.clone(),
            development: true,
        });
        let path = local.path().to_owned();
        let worker =
            tokio::spawn(async move { queue::operations(client, &path, requests, events).await });
        assert!(matches!(
            updates.recv().await.unwrap(),
            OperationEvent::Ready
        ));
        writes
            .send(operation(command.clone()).into())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), fixture.observed.recv())
            .await
            .unwrap()
            .unwrap();
        // The local association must already be on disk when the Agent executes.
        let persisted: Vec<Operation> =
            serde_json::from_slice(&std::fs::read(local.path().join("operations.json")).unwrap())
                .unwrap();
        let receipt = persisted[0].receipt.clone().unwrap();
        let ExecutionReceipt::Remote {
            request_id,
            agent,
            owner,
            ..
        } = &receipt;
        assert_eq!(agent, &fixture.agent.public_key().id());
        assert!(
            fixture
                .store
                .operation(request_id, owner)
                .unwrap()
                .is_some()
        );
        worker.abort();
        assert!(worker.await.unwrap_err().is_cancelled());
        drop(writes);
        let restored = tokio::time::timeout(
            Duration::from_secs(1),
            OperationService::open(fixture.client.clone(), local.path().to_owned()),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            restored.get(301).unwrap().state,
            OperationState::Reconciling
        );
        assert_eq!(
            restored
                .submit(301, command)
                .await
                .unwrap()
                .receipt
                .as_ref(),
            Some(&receipt)
        );
        fixture.provider.release.add_permits(1);
        let recovered = done(&restored, 301).await;
        assert_eq!(recovered.state, OperationState::Succeeded);
        assert!(recovered.result.is_some());
        assert_eq!(recovered.receipt, Some(receipt));
        assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.store.operation_records().unwrap().len(), 1);
        restored.shutdown().await;
    }
}

#[tokio::test]
async fn missing_agent_record_stays_unknown_and_manual_reconcile_recovers_after_expiry() {
    let fixture = Fixture::new(false).await;
    let local = tempfile::tempdir().unwrap();
    let command = fixture.command().await;
    let mut op = operation(command.clone());
    op.receipt = fixture.client.prepare_execution(&command).unwrap();
    // Simulate crash after local Running was saved, before Submit reached the Agent.
    op.state = OperationState::Running;
    if let Some(ExecutionReceipt::Remote { expires_at, .. }) = &mut op.receipt {
        *expires_at = now() - 1;
    }
    save_json(&local.path().join("operations.json"), &vec![op.clone()]).unwrap();
    let service = OperationService::open(fixture.client.clone(), local.path().to_owned())
        .await
        .unwrap();
    assert_eq!(
        done(&service, 301).await.state,
        OperationState::UnknownOutcome
    );
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
    assert!(fixture.store.operation_records().unwrap().is_empty());
    // An authoritative historical record arrives later; expiry prevents submission, not queries.
    let ExecutionReceipt::Remote {
        agent,
        owner,
        request_id,
        expires_at,
    } = op.receipt.clone().unwrap();
    let request = Mutation {
        request_id: request_id.clone(),
        command,
        expires_at,
    };
    let record = RemoteOperation {
        request: request.clone(),
        agent,
        owner: owner.clone(),
        device: "original-device".into(),
        accepted_at: now() - 2,
        state: OperationState::Succeeded,
        error: None,
        result: Some(CommandResult::Confirmed),
        progress: None,
        cancel_requested: false,
    };
    fixture
        .store
        .accept_operation(
            &request_id,
            &owner,
            &serde_json::to_string(&request).unwrap(),
            &serde_json::to_string(&record).unwrap(),
        )
        .unwrap();
    service.reconcile(301).await.unwrap();
    assert_eq!(done(&service, 301).await.state, OperationState::Succeeded);
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
    service.shutdown().await;
}

#[tokio::test]
async fn recovery_rejects_wrong_binding_and_mismatched_authoritative_record() {
    let fixture = Fixture::new(false).await;
    let command = fixture.command().await;
    let receipt = fixture.client.prepare_execution(&command).unwrap().unwrap();
    let mut wrong = receipt.clone();
    let ExecutionReceipt::Remote { agent, .. } = &mut wrong;
    *agent = SecretKey::generate().public().id();
    assert!(
        fixture
            .client
            .resume_execution(&command, &wrong, &ExecutionControl::default())
            .await
            .is_err()
    );
    let mut wrong = receipt.clone();
    let ExecutionReceipt::Remote { owner, .. } = &mut wrong;
    *owner = SecretKey::generate().public().id();
    assert!(
        fixture
            .client
            .resume_execution(&command, &wrong, &ExecutionControl::default())
            .await
            .is_err()
    );
    let ExecutionReceipt::Remote {
        agent,
        owner,
        request_id,
        expires_at,
    } = &receipt;
    let request = Mutation {
        request_id: request_id.clone(),
        command: command.clone(),
        expires_at: *expires_at,
    };
    let mut record = RemoteOperation {
        request: request.clone(),
        agent: agent.clone(),
        owner: owner.clone(),
        device: "device".into(),
        accepted_at: now(),
        state: OperationState::Succeeded,
        error: None,
        result: Some(CommandResult::Confirmed),
        progress: None,
        cancel_requested: false,
    };
    fixture
        .store
        .accept_operation(
            request_id,
            owner,
            &serde_json::to_string(&request).unwrap(),
            &serde_json::to_string(&record).unwrap(),
        )
        .unwrap();
    record.owner = "different-owner".into();
    fixture
        .store
        .finish_operation(request_id, &serde_json::to_string(&record).unwrap())
        .unwrap();
    assert!(
        fixture
            .client
            .resume_execution(&command, &receipt, &ExecutionControl::default())
            .await
            .is_err()
    );
    record.owner = owner.clone();
    record.request.expires_at += 1;
    fixture
        .store
        .finish_operation(request_id, &serde_json::to_string(&record).unwrap())
        .unwrap();
    assert!(
        fixture
            .client
            .resume_execution(&command, &receipt, &ExecutionControl::default())
            .await
            .is_err()
    );
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn recovery_checks_current_read_permission_and_preserves_agent_restart_uncertainty() {
    let fixture = Fixture::new(false).await;
    let command = fixture.command().await;
    let receipt = fixture.client.prepare_execution(&command).unwrap().unwrap();
    let ExecutionReceipt::Remote {
        agent,
        owner,
        request_id,
        expires_at,
    } = &receipt;
    let request = Mutation {
        request_id: request_id.clone(),
        command: command.clone(),
        expires_at: *expires_at,
    };
    let record = RemoteOperation {
        request: request.clone(),
        agent: agent.clone(),
        owner: owner.clone(),
        device: "device".into(),
        accepted_at: now(),
        state: OperationState::Running,
        error: None,
        result: None,
        progress: None,
        cancel_requested: false,
    };
    fixture
        .store
        .accept_operation(
            request_id,
            owner,
            &serde_json::to_string(&request).unwrap(),
            &serde_json::to_string(&record).unwrap(),
        )
        .unwrap();
    let restricted = Arc::new(RemoteClient {
        endpoint: fixture.direct.clone(),
        pin: fixture.client.pin.clone(),
        development: true,
        credentials: Arc::new(credentials(
            &fixture.owner,
            SecretKey::generate(),
            &fixture.issuer,
            Some(fixture.client.credentials.latest_policy().unwrap()),
            vec![Permission::Lifecycle],
        )),
    });
    assert!(
        restricted
            .resume_execution(&command, &receipt, &ExecutionControl::default())
            .await
            .is_err()
    );
    let local = tempfile::tempdir().unwrap();
    let mut op = operation(command);
    op.state = OperationState::Running;
    op.receipt = Some(receipt);
    save_json(&local.path().join("operations.json"), &vec![op]).unwrap();
    let service = OperationService::open(restricted, local.path().to_owned())
        .await
        .unwrap();
    assert_eq!(
        done(&service, 301).await.state,
        OperationState::UnknownOutcome,
        "permission denial is not proof of failed execution"
    );
    service.shutdown().await;
    let _reloaded = Agent::new(
        fixture.store.clone(),
        SecretKey::from_bytes(&fixture.agent_key.secret_bytes()).unwrap(),
        fixture.provider.clone(),
        "demo".into(),
    )
    .unwrap();
    let service = OperationService::open(fixture.client.clone(), local.path().to_owned())
        .await
        .unwrap();
    assert_eq!(
        done(&service, 301).await.state,
        OperationState::UnknownOutcome,
        "an interrupted Agent execution cannot be guessed from container state"
    );
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.store.operation_records().unwrap().len(), 1);
    service.shutdown().await;
}
