use super::*;

#[tokio::test]
async fn host_runtime_start_is_rejected_by_both_client_and_agent() {
    let fixture = Fixture::new(false).await;
    // This transport fixture uses the unscoped demo environment; inspect the
    // server response directly (the production client requires an agent prefix).
    let Response::Diagnostics(report) =
        fixture.client.request(&Request::Diagnostics).await.unwrap()
    else {
        panic!("expected diagnostics");
    };
    assert!(!report.supports(RuntimeFeature::StartRuntime));
    let command = Command::StartRuntime {
        environment: "demo".into(),
    };
    assert_eq!(
        fixture.client.execute(&command).await.unwrap_err().kind,
        ErrorKind::Unsupported
    );
    let response = fixture
        .client
        .request(&Request::Submit(Mutation {
            request_id: nonce(),
            command,
            expires_at: now() + 120,
        }))
        .await
        .unwrap();
    assert!(matches!(response, Response::Error(_)));
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
    assert!(fixture.store.operation_records().unwrap().is_empty());
}

async fn create(fixture: &Fixture) -> (Command, SecretEnvironment) {
    let image = fixture.provider.inner.snapshot().await.unwrap().containers[0]
        .image
        .clone();
    let command = Command::CreateContainer {
        environment: "demo".into(),
        spec: CreateContainer {
            name: "permission-fixture".into(),
            image,
            cpus: 1,
            memory_mib: 256,
            ports: vec![],
            options: CreateOptions {
                start_after_create: true,
                secret_environment: vec!["API_TOKEN".into()],
                ..Default::default()
            },
        },
    };
    let mut input = SecretEnvironment::default();
    input
        .insert("API_TOKEN".into(), "remote-injection-canary".into())
        .unwrap();
    (command, input)
}

#[tokio::test]
async fn create_and_start_requires_both_permissions_before_creating() {
    let fixture = Fixture::new(false).await;
    let restricted = RemoteClient {
        endpoint: fixture.direct.clone(),
        pin: fixture.client.pin.clone(),
        development: true,
        credentials: Arc::new(credentials(
            &fixture.owner,
            SecretKey::generate(),
            &fixture.issuer,
            Some(fixture.client.credentials.latest_policy().unwrap()),
            vec![Permission::Read, Permission::ManageContainers],
        )),
    };
    let snapshot = restricted.snapshot().await.unwrap();
    assert!(snapshot.capabilities.create_container);
    assert!(!snapshot.capabilities.create_and_start);
    let (command, input) = create(&fixture).await;
    assert_eq!(
        restricted
            .execute_controlled(&command, &ExecutionControl::with_secrets(input))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::PermissionDenied
    );
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
    assert!(fixture.store.operation_records().unwrap().is_empty());
}

#[tokio::test]
async fn rebuild_requires_manage_and_lifecycle_before_queueing() {
    let fixture = Fixture::new(false).await;
    for permission in [Permission::ManageContainers, Permission::Lifecycle] {
        let restricted = RemoteClient {
            endpoint: fixture.direct.clone(),
            pin: fixture.client.pin.clone(),
            development: true,
            credentials: Arc::new(credentials(
                &fixture.owner,
                SecretKey::generate(),
                &fixture.issuer,
                Some(fixture.client.credentials.latest_policy().unwrap()),
                vec![Permission::Read, permission],
            )),
        };
        let snapshot = restricted.snapshot().await.unwrap();
        assert!(!snapshot.capabilities.rebuild_ports);
        let old = &snapshot.containers[0];
        let command = Command::RebuildPorts {
            key: old.key.clone(),
            expected_state: old.state.clone(),
            expected: old.ports.clone(),
            ports: vec![],
        };
        assert_eq!(
            restricted.execute(&command).await.unwrap_err().kind,
            ErrorKind::PermissionDenied
        );
    }
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
    assert!(fixture.store.operation_records().unwrap().is_empty());
}

#[tokio::test]
async fn revoked_during_creation_retains_partial_result_and_never_starts() {
    let mut fixture = Fixture::new(false).await;
    let (command, input) = create(&fixture).await;
    let request = Mutation {
        request_id: nonce(),
        command: command.clone(),
        expires_at: now() + 900,
    };
    let Response::Operation(initial) = fixture
        .client
        .request(&Request::SubmitWithSecrets {
            request: request.clone(),
            secrets: input,
        })
        .await
        .unwrap()
    else {
        panic!("not accepted")
    };
    tokio::time::timeout(Duration::from_secs(3), fixture.observed.recv())
        .await
        .unwrap()
        .unwrap();
    let policy = fixture
        .owner
        .sign(
            POLICY_DOMAIN,
            &OwnerPolicy {
                version: VERSION,
                owner: fixture.owner.public(),
                revision: 2,
                revoked_devices: vec![initial.device],
                issued_at: now(),
                expires_at: now() + 3600,
            },
        )
        .unwrap();
    let admin = RemoteClient {
        endpoint: fixture.direct.clone(),
        pin: fixture.client.pin.clone(),
        development: true,
        credentials: Arc::new(credentials(
            &fixture.owner,
            SecretKey::generate(),
            &fixture.issuer,
            Some(policy),
            vec![Permission::Read],
        )),
    };
    admin.snapshot().await.unwrap(); // Authenticated newer owner policy revokes the submitting device.
    fixture.provider.release.add_permits(1);
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let (_, json) = fixture
                .store
                .operation(&request.request_id, &fixture.owner.public().id())
                .unwrap()
                .unwrap();
            let op: RemoteOperation = serde_json::from_str(&json).unwrap();
            if !op.state.is_active() {
                break op;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(result.state, OperationState::Failed);
    assert_eq!(
        result.error.as_ref().unwrap().kind,
        ErrorKind::PermissionDenied
    );
    let Some(CommandResult::Container(container)) = &result.result else {
        panic!("lost created identity")
    };
    assert_eq!(container.state, ContainerState::Stopped);
    assert_eq!(
        fixture.provider.calls.load(Ordering::SeqCst),
        1,
        "revoked start must not reach provider"
    );
    let receipt = ExecutionReceipt::Remote {
        agent: result.agent.clone(),
        owner: result.owner.clone(),
        request_id: request.request_id.clone(),
        expires_at: request.expires_at,
    };
    let recovered = admin
        .resume_execution(&command, &receipt, &ExecutionControl::default())
        .await
        .unwrap();
    assert_eq!(recovered.state, OperationState::Failed);
    assert!(recovered.result.is_some());

    // Durable phase fixture for an Agent crash between saving creation and completing start.
    let mut interrupted = result;
    interrupted.state = OperationState::Running;
    interrupted.error = None;
    fixture
        .store
        .finish_operation(
            &request.request_id,
            &serde_json::to_string(&interrupted).unwrap(),
        )
        .unwrap();
    let _reloaded = Agent::new(
        fixture.store.clone(),
        SecretKey::from_bytes(&fixture.agent_key.secret_bytes()).unwrap(),
        fixture.provider.clone(),
        "demo".into(),
    )
    .unwrap();
    let recovered = admin
        .resume_execution(&command, &receipt, &ExecutionControl::default())
        .await
        .unwrap();
    assert_eq!(recovered.state, OperationState::UnknownOutcome);
    assert!(
        matches!(recovered.result, Some(CommandResult::Container(c)) if c.key == container_key(&interrupted))
    );
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 1);
}
fn container_key(op: &RemoteOperation) -> ResourceKey {
    let Some(CommandResult::Container(container)) = &op.result else {
        panic!("container")
    };
    container.key.clone()
}
