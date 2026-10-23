#[path = "transports/cancellation.rs"]
mod cancellation;
#[path = "transports/recovery.rs"]
mod recovery;

use containerhub_core::{environment::EnvironmentClient, *};
use containerhub_remote::{
    client::*,
    crypto::*,
    identity::*,
    relay,
    rpc::{Mutation, Request, Response},
    server::Agent,
    store::*,
};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

fn credentials(
    owner: &SecretKey,
    device: SecretKey,
    issuer: &SecretKey,
    policy: Option<Signed>,
    permissions: Vec<Permission>,
) -> Credentials {
    let time = now();
    let grant = owner
        .sign(
            GRANT_DOMAIN,
            &DeviceGrant {
                version: VERSION,
                owner: owner.public(),
                device: device.public(),
                serial: nonce(),
                agents: Scope::All,
                environments: Scope::All,
                permissions,
                not_before: time,
                expires_at: time + 3600,
            },
        )
        .unwrap();
    let policy = policy.unwrap_or_else(|| {
        owner
            .sign(
                POLICY_DOMAIN,
                &OwnerPolicy {
                    version: VERSION,
                    owner: owner.public(),
                    revision: 1,
                    revoked_devices: vec![],
                    issued_at: time,
                    expires_at: time + 3600,
                },
            )
            .unwrap()
    });
    let app_credential = Some(
        issuer
            .sign(
                APP_DOMAIN,
                &AppCredential {
                    version: VERSION,
                    device: device.public(),
                    audience: "test".into(),
                    not_before: time,
                    expires_at: time + 3600,
                },
            )
            .unwrap(),
    );
    Credentials::new(device, grant, policy, app_credential, None)
}
#[tokio::test]
async fn direct_and_outbound_relay_share_owner_pin_and_authorization() {
    let directory = tempfile::tempdir().unwrap();
    let agent_key = SecretKey::generate();
    let issuer = SecretKey::generate();
    let owner = SecretKey::generate();
    let store = Arc::new(
        BindingStore::initialize(
            directory.path(),
            Installation {
                version: VERSION,
                agent: agent_key.public(),
                app_issuer: issuer.public(),
                audience: "test".into(),
            },
        )
        .unwrap(),
    );
    let fake = Arc::new(fake::FakeProvider::default());
    let local = fake.snapshot().await.unwrap().environment;
    let environment = format!("agent:{}:apple:current-user", agent_key.public().id());
    let provider = Arc::new(EnvironmentClient::new(
        fake,
        local,
        environment.clone(),
        ProviderKind::Apple,
    ));
    let pin = agent_key.public();
    let agent = Agent::new(store, agent_key, provider, environment).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let direct = tokio::spawn(agent.clone().serve(listener));
    let endpoint = Endpoint::Direct { url };
    let native = Arc::new(credentials(
        &owner,
        SecretKey::generate(),
        &issuer,
        None,
        vec![
            Permission::Read,
            Permission::Logs,
            Permission::Lifecycle,
            Permission::ManageTrust,
        ],
    ));
    let (_, receipt) = authenticate(&endpoint, &native, None, true, true)
        .await
        .unwrap();
    assert_eq!(receipt.owner, owner.public());
    let direct_client = RemoteClient {
        endpoint: endpoint.clone(),
        credentials: native.clone(),
        pin: pin.clone(),
        development: true,
    };
    let original = direct_client.snapshot().await.unwrap();
    assert!(original.environment.starts_with("agent:"));
    let host = direct_client.host_snapshot().await.unwrap();
    assert_eq!(host.environment, original.environment);
    assert!(host.sample);
    assert!(host.memory_total_bytes.is_some());
    let mut wrong = original.containers[0].key.clone();
    wrong.provider = ProviderKind::Kubernetes;
    let denied = direct_client
        .request(&Request::Logs {
            key: wrong,
            source: LogSource::Standard,
        })
        .await
        .unwrap();
    assert!(matches!(denied, Response::Error(_)));

    let relay_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_url = format!("ws://{}", relay_listener.local_addr().unwrap());
    let relay_server = tokio::spawn(relay::serve(relay_listener));
    let mut outbound = vec![];
    for _ in 0..4 {
        outbound.push(tokio::spawn(relay::outbound(
            agent.clone(),
            relay_url.clone(),
            true,
        )));
    }
    let relayed = Endpoint::Relay {
        url: relay_url.clone(),
        route: pin.id(),
    };
    let relay_client = RemoteClient {
        endpoint: relayed,
        credentials: native.clone(),
        pin: pin.clone(),
        development: true,
    };
    let mut connected = None;
    for _ in 0..30 {
        match relay_client.snapshot().await {
            Ok(snapshot) => {
                connected = Some(snapshot);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
    assert_eq!(
        connected.expect("outbound relay connected").environment,
        original.environment
    );
    assert_eq!(
        relay_client.host_snapshot().await.unwrap().environment,
        original.environment
    );
    let automatic = RemoteClient {
        endpoint: Endpoint::Auto {
            direct: "ws://127.0.0.1:1".into(),
            relay: relay_url.clone(),
            route: pin.id(),
        },
        credentials: native.clone(),
        pin: pin.clone(),
        development: true,
    };
    assert_eq!(
        automatic.host_snapshot().await.unwrap().environment,
        original.environment
    );
    let denied = RemoteClient {
        endpoint: endpoint.clone(),
        credentials: Arc::new(credentials(
            &owner,
            SecretKey::generate(),
            &issuer,
            Some(native.latest_policy().unwrap()),
            vec![Permission::Logs],
        )),
        pin: pin.clone(),
        development: true,
    };
    assert_eq!(
        denied.host_snapshot().await.unwrap_err().kind,
        ErrorKind::PermissionDenied
    );
    let wrong_pin = RemoteClient {
        endpoint: endpoint.clone(),
        credentials: native.clone(),
        pin: SecretKey::generate().public(),
        development: true,
    };
    assert!(wrong_pin.host_snapshot().await.is_err());
    let key = original.containers[0].key.clone();
    relay_client
        .lifecycle(&key, LifecycleAction::Stop)
        .await
        .unwrap();
    assert_eq!(
        direct_client
            .snapshot()
            .await
            .unwrap()
            .containers
            .iter()
            .find(|c| c.key == key)
            .unwrap()
            .state,
        ContainerState::Stopped
    );
    assert!(
        !relay_client
            .logs(&key, LogSource::Standard)
            .await
            .unwrap()
            .lines
            .is_empty()
    );

    let browser = Arc::new(credentials(
        &owner,
        SecretKey::generate(),
        &issuer,
        Some(native.latest_policy().unwrap()),
        vec![Permission::Read, Permission::Logs],
    ));
    let browser_client = RemoteClient {
        endpoint: endpoint.clone(),
        credentials: browser,
        pin: pin.clone(),
        development: true,
    };
    assert!(!browser_client.snapshot().await.unwrap().lifecycle_supported);
    let request = Request::Submit(Mutation {
        request_id: nonce(),
        command: Command::Lifecycle {
            key: key.clone(),
            action: LifecycleAction::Start,
        },
        expires_at: now() + 60,
    });
    assert!(matches!(
        browser_client.request(&request).await.unwrap(),
        Response::Error(ManagementError {
            kind: ErrorKind::PermissionDenied,
            ..
        })
    ));

    let stranger = credentials(
        &SecretKey::generate(),
        SecretKey::generate(),
        &issuer,
        None,
        vec![Permission::Read, Permission::ManageTrust],
    );
    assert!(
        authenticate(&endpoint, &stranger, None, true, true)
            .await
            .is_err()
    );
    assert!(
        authenticate(
            &endpoint,
            &native,
            Some(&SecretKey::generate().public()),
            false,
            true
        )
        .await
        .is_err()
    );
    // A repeat submission returns the same outcome and a changed payload cannot reuse that ID.
    let mutation = Mutation {
        request_id: nonce(),
        command: Command::Lifecycle {
            key,
            action: LifecycleAction::Start,
        },
        expires_at: now() + 60,
    };
    let first = direct_client
        .request(&Request::Submit(mutation.clone()))
        .await
        .unwrap();
    assert!(matches!(first, Response::Operation(_)));
    for _ in 0..20 {
        let response = direct_client
            .request(&Request::Operation {
                request_id: mutation.request_id.clone(),
            })
            .await
            .unwrap();
        if matches!(
            response,
            Response::Operation(RemoteOperation {
                state: OperationState::Succeeded,
                ..
            })
        ) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(matches!(
        direct_client
            .request(&Request::Submit(mutation.clone()))
            .await
            .unwrap(),
        Response::Operation(_)
    ));
    let mut conflict = mutation;
    if let Command::Lifecycle { action, .. } = &mut conflict.command {
        *action = LifecycleAction::Stop;
    }
    assert!(matches!(
        direct_client
            .request(&Request::Submit(conflict))
            .await
            .unwrap(),
        Response::Error(_)
    ));
    direct.abort();
    relay_server.abort();
    for task in outbound {
        task.abort();
    }
}

use containerhub_remote::rpc::RemoteOperation;

struct ControlledProvider {
    inner: fake::FakeProvider,
    release: tokio::sync::Semaphore,
    entered: tokio::sync::mpsc::UnboundedSender<ResourceKey>,
    active: std::sync::Mutex<std::collections::HashSet<ResourceKey>>,
    calls: std::sync::atomic::AtomicUsize,
}
#[async_trait::async_trait]
impl ManagementClient for ControlledProvider {
    async fn diagnostics(&self) -> Result<RuntimeDiagnostics, ManagementError> {
        self.inner.diagnostics().await
    }
    async fn execute_controlled(
        &self,
        command: &Command,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        if !command.secret_names().is_empty() {
            control.secrets().validate_for(command)?;
            assert_eq!(
                &**control.secrets().stdin_bytes().unwrap(),
                b"API_TOKEN=remote-injection-canary\n"
            );
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.entered
                .send(self.inner.snapshot().await?.containers[0].key.clone())
                .unwrap();
            self.release.acquire().await.unwrap().forget();
            return self.inner.execute_controlled(command, control).await;
        }
        if command.cancellable() {
            control.check_before_start()?;
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            control.report(OperationProgress::Fetching);
            while !control.is_cancelled() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(ManagementError::new(
                ErrorKind::UnknownOutcome,
                "runtime outcome requires reconciliation",
            ))
        } else {
            self.execute(command).await
        }
    }
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        self.inner.snapshot().await
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
    async fn lifecycle(
        &self,
        key: &ResourceKey,
        action: LifecycleAction,
    ) -> Result<(), ManagementError> {
        assert!(
            self.active.lock().unwrap().insert(key.clone()),
            "same resource executed concurrently"
        );
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.entered.send(key.clone()).unwrap();
        self.release.acquire().await.unwrap().forget();
        let result = self.inner.lifecycle(key, action).await;
        assert!(self.active.lock().unwrap().remove(key));
        result
    }
}

#[tokio::test]
async fn remote_queue_bounds_concurrency_serializes_resources_and_deduplicates_while_busy() {
    let directory = tempfile::tempdir().unwrap();
    let agent_key = SecretKey::generate();
    let issuer = SecretKey::generate();
    let owner = SecretKey::generate();
    let store = Arc::new(
        BindingStore::initialize(
            directory.path(),
            Installation {
                version: VERSION,
                agent: agent_key.public(),
                app_issuer: issuer.public(),
                audience: "test".into(),
            },
        )
        .unwrap(),
    );
    let (entered, mut observed) = tokio::sync::mpsc::unbounded_channel();
    let provider = Arc::new(ControlledProvider {
        inner: fake::FakeProvider::default(),
        release: tokio::sync::Semaphore::new(0),
        entered,
        active: Default::default(),
        calls: Default::default(),
    });
    let snapshot = provider.snapshot().await.unwrap();
    let pin = agent_key.public();
    let agent = Agent::new(store, agent_key, provider.clone(), snapshot.environment).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = Endpoint::Direct {
        url: format!("ws://{}", listener.local_addr().unwrap()),
    };
    let serving = tokio::spawn(agent.serve(listener));
    let native = Arc::new(credentials(
        &owner,
        SecretKey::generate(),
        &issuer,
        None,
        vec![
            Permission::Read,
            Permission::Lifecycle,
            Permission::ManageTrust,
        ],
    ));
    authenticate(&endpoint, &native, None, true, true)
        .await
        .unwrap();
    let client = RemoteClient {
        endpoint,
        credentials: native,
        pin,
        development: true,
    };
    let mut requests = vec![];
    for index in [0, 1, 2, 3, 0, 4] {
        let mutation = Mutation {
            request_id: nonce(),
            command: Command::Lifecycle {
                key: snapshot.containers[index].key.clone(),
                action: LifecycleAction::Stop,
            },
            expires_at: now() + 60,
        };
        assert!(matches!(
            client
                .request(&Request::Submit(mutation.clone()))
                .await
                .unwrap(),
            Response::Operation(_)
        ));
        requests.push(mutation);
    }
    for _ in 0..4 {
        tokio::time::timeout(Duration::from_secs(2), observed.recv())
            .await
            .unwrap()
            .unwrap();
    }
    assert!(observed.try_recv().is_err());
    // Dedupe must work even while all four execution slots are occupied.
    assert!(matches!(
        client
            .request(&Request::Submit(requests[0].clone()))
            .await
            .unwrap(),
        Response::Operation(_)
    ));
    assert_eq!(provider.calls.load(std::sync::atomic::Ordering::SeqCst), 4);
    provider.release.add_permits(6);
    tokio::time::timeout(Duration::from_secs(5), async {
        for request in &requests {
            loop {
                let response = client
                    .request(&Request::Operation {
                        request_id: request.request_id.clone(),
                    })
                    .await
                    .unwrap();
                if matches!(
                    response,
                    Response::Operation(RemoteOperation {
                        state: OperationState::Succeeded,
                        ..
                    })
                ) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(provider.calls.load(std::sync::atomic::Ordering::SeqCst), 6);
    assert!(provider.active.lock().unwrap().is_empty());
    serving.abort();
}
