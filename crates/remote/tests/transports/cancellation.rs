use super::*;

#[tokio::test]
async fn remote_cancel_checks_permissions_and_propagates_progress_without_replay() {
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
    let (entered, _) = tokio::sync::mpsc::unbounded_channel();
    let provider = Arc::new(ControlledProvider {
        inner: fake::FakeProvider::default(),
        release: tokio::sync::Semaphore::new(0),
        entered,
        active: Default::default(),
        calls: Default::default(),
    });
    let pin = agent_key.public();
    let agent = Agent::new(store, agent_key, provider.clone(), "demo".into()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = Endpoint::Direct {
        url: format!("ws://{}", listener.local_addr().unwrap()),
    };
    let server = tokio::spawn(agent.clone().serve(listener));
    let native = Arc::new(credentials(
        &owner,
        SecretKey::generate(),
        &issuer,
        None,
        vec![
            Permission::Read,
            Permission::ManageImages,
            Permission::ManageTrust,
        ],
    ));
    authenticate(&endpoint, &native, None, true, true)
        .await
        .unwrap();
    let client = RemoteClient {
        endpoint: endpoint.clone(),
        credentials: native.clone(),
        pin: pin.clone(),
        development: true,
    };
    let read = RemoteClient {
        endpoint: endpoint.clone(),
        credentials: Arc::new(credentials(
            &owner,
            SecretKey::generate(),
            &issuer,
            Some(native.latest_policy().unwrap()),
            vec![Permission::Read],
        )),
        pin: pin.clone(),
        development: true,
    };
    let command = Command::PullImage {
        environment: "demo".into(),
        reference: "example.invalid/cancel:v1".into(),
    };
    let first = Mutation {
        request_id: nonce(),
        command: command.clone(),
        expires_at: now() + 60,
    };
    client
        .request(&Request::Submit(first.clone()))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Response::Operation(op) = client
                .request(&Request::Operation {
                    request_id: first.request_id.clone(),
                })
                .await
                .unwrap()
                && op.progress == Some(OperationProgress::Fetching)
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    let second = Mutation {
        request_id: nonce(),
        ..first.clone()
    };
    client
        .request(&Request::Submit(second.clone()))
        .await
        .unwrap();
    assert!(matches!(
        read.request(&Request::Cancel {
            request_id: first.request_id.clone()
        })
        .await
        .unwrap(),
        Response::Error(ManagementError {
            kind: ErrorKind::PermissionDenied,
            ..
        })
    ));
    assert!(
        matches!(client.request(&Request::Cancel { request_id: second.request_id.clone() }).await.unwrap(), Response::Operation(op) if op.state == OperationState::Cancelled && op.cancel_requested)
    );
    client
        .request(&Request::Cancel {
            request_id: first.request_id.clone(),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Response::Operation(op) = client
                .request(&Request::Operation {
                    request_id: first.request_id.clone(),
                })
                .await
                .unwrap()
                && !op.state.is_active()
            {
                assert_eq!(op.state, OperationState::UnknownOutcome);
                assert!(op.cancel_requested);
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(
        matches!(client.request(&Request::Submit(second)).await.unwrap(), Response::Operation(op) if op.state == OperationState::Cancelled)
    );
    assert_eq!(provider.calls.load(std::sync::atomic::Ordering::SeqCst), 1);

    // The public controlled provider API must carry both progress and cancellation through an outbound relay.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let relay_server = tokio::spawn(relay::serve(listener));
    let outbound: Vec<_> = (0..4)
        .map(|_| tokio::spawn(relay::outbound(agent.clone(), url.clone(), true)))
        .collect();
    let relay_client = RemoteClient {
        endpoint: Endpoint::Relay {
            url,
            route: pin.id(),
        },
        credentials: native,
        pin,
        development: true,
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    let control = ExecutionControl::default();
    let cancel = async {
        while control.progress() != Some(OperationProgress::Fetching) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        control.cancel();
    };
    let (result, _) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(relay_client.execute_controlled(&command, &control), cancel)
    })
    .await
    .unwrap();
    assert_eq!(result.unwrap_err().kind, ErrorKind::UnknownOutcome);
    assert_eq!(provider.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    for task in outbound {
        task.abort();
    }
    relay_server.abort();
    server.abort();
}
