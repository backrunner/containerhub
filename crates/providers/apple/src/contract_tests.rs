use super::*;

pub(super) fn fixture() -> (tempfile::TempDir, AppleProvider) {
    use std::os::unix::fs::PermissionsExt;
    let temporary = tempfile::tempdir().unwrap();
    let binary = temporary.path().join("container-fixture");
    std::fs::write(
        &binary,
        include_bytes!("../../../../tests/fixtures/apple/cli_fixture.py"),
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let provider = AppleProvider {
        discovery: Default::default(),
        image_writes: Default::default(),
        runtime_starts: Default::default(),
        binary: Some(binary),
        samples: Default::default(),
        host: Default::default(),
    };
    (temporary, provider)
}

#[tokio::test]
async fn controlled_pull_streams_stages_and_cancellation_is_not_a_rollback() {
    let (temporary, provider) = fixture();
    let state = temporary.path().join("fixture-state.json");
    std::fs::write(
        &state,
        r#"{"images":[],"containers":[],"sequence":0,"pull_delay":2,"unpack_delay":0.1}"#,
    )
    .unwrap();
    let control = ExecutionControl::default();
    let command = Command::PullImage {
        environment: LOCAL_ENVIRONMENT.into(),
        reference: "example.invalid/slow:v1".into(),
    };
    let cancel = async {
        tokio::time::timeout(Duration::from_secs(5), async {
            while control.progress() != Some(OperationProgress::Fetching) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        control.cancel();
    };
    let (result, _) = tokio::join!(provider.execute_controlled(&command, &control), cancel);
    assert_eq!(result.unwrap_err().kind, ErrorKind::UnknownOutcome);
    assert!(
        provider
            .resources(ResourceKind::Image)
            .await
            .unwrap()
            .is_empty()
    );
    let before = std::fs::read(temporary.path().join("arguments.jsonl")).unwrap();
    assert_eq!(
        provider
            .execute_controlled(&command, &control)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    // Cancelled before acquiring the mutation gate never starts image pull again.
    let after = std::fs::read_to_string(temporary.path().join("arguments.jsonl")).unwrap();
    assert_eq!(
        after.matches("\"pull\"").count(),
        String::from_utf8(before)
            .unwrap()
            .matches("\"pull\"")
            .count()
    );
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
    config["pull_delay"] = 0.into();
    config["unpack_delay"] = 0.2.into();
    std::fs::write(&state, serde_json::to_vec(&config).unwrap()).unwrap();
    let control = ExecutionControl::default();
    let observe = async {
        tokio::time::timeout(Duration::from_secs(5), async {
            while control.progress() != Some(OperationProgress::Unpacking) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    };
    let (result, _) = tokio::join!(provider.execute_controlled(&command, &control), observe);
    result.unwrap();
    assert_eq!(control.progress(), Some(OperationProgress::Verifying));
}

#[tokio::test]
async fn advanced_create_and_fresh_inspect_use_only_typed_public_options() {
    let (temporary, provider) = fixture();
    provider
        .execute(&Command::PullImage {
            environment: LOCAL_ENVIRONMENT.into(),
            reference: "example.invalid/advanced:v1".into(),
        })
        .await
        .unwrap();
    let mut spec = CreateContainer {
        name: "advanced".into(),
        image: "example.invalid/advanced:v1".into(),
        cpus: 1,
        memory_mib: 256,
        ports: vec![],
        options: CreateOptions {
            start_after_create: false,
            startup: None,
            secret_environment: vec![],
            networks: vec!["default".into()],
            volumes: vec![VolumeMount {
                name: "database-data".into(),
                destination: "/data".into(),
                read_only: true,
            }],
            user: Some("1000:1000".into()),
            working_directory: Some("/app".into()),
            read_only: true,
        },
    };
    let CommandResult::Container(container) = provider
        .execute(&Command::CreateContainer {
            environment: LOCAL_ENVIRONMENT.into(),
            spec: spec.clone(),
        })
        .await
        .unwrap()
    else {
        panic!("container")
    };
    let details = provider.inspect_container(&container.key).await.unwrap();
    assert_eq!(details.read_only, Some(true));
    assert_eq!(details.working_directory.as_deref(), Some("/app"));
    assert!(
        !serde_json::to_string(&details)
            .unwrap()
            .contains("fixture-secret")
    );
    spec.name = "missing-volume".into();
    spec.options.volumes[0].name = "missing".into();
    assert_eq!(
        provider
            .execute(&Command::CreateContainer {
                environment: LOCAL_ENVIRONMENT.into(),
                spec
            })
            .await
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    );
    let calls = std::fs::read_to_string(temporary.path().join("arguments.jsonl")).unwrap();
    let calls: Vec<Vec<String>> = calls
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let creates: Vec<_> = calls
        .iter()
        .filter(|args| args[0] == "create" && !args.iter().any(|arg| arg == "--help"))
        .collect();
    assert_eq!(creates.len(), 1);
    assert!(creates[0].contains(&"type=volume,source=database-data,target=/data,readonly".into()));
    assert!(creates[0].contains(&"--read-only".into()));
    assert!(creates[0].contains(&"1000:1000".into()));
}

#[tokio::test]
async fn public_cli_lifecycle_checks_arguments_identity_and_actual_postconditions() {
    let (temporary, provider) = fixture();
    let environment = LOCAL_ENVIRONMENT.to_string();
    let reference = "example.invalid/contract-fixture:v1".to_string();
    provider
        .execute(&Command::PullImage {
            environment: environment.clone(),
            reference: reference.clone(),
        })
        .await
        .unwrap();
    let spec = CreateContainer {
        options: Default::default(),
        name: "contract-test".into(),
        image: reference,
        cpus: 1,
        memory_mib: 256,
        ports: vec![Port {
            host: "127.0.0.1".into(),
            host_port: 18080,
            container_port: 80,
            protocol: "tcp".into(),
        }],
    };
    let command = Command::CreateContainer { environment, spec };
    let CommandResult::Container(created) = provider.execute(&command).await.unwrap() else {
        panic!("created identity")
    };
    assert_eq!(created.state, ContainerState::Stopped);
    assert_eq!(
        provider.execute(&command).await.unwrap_err().kind,
        ErrorKind::Conflict
    );
    let key = created.key;
    provider
        .lifecycle(&key, LifecycleAction::Start)
        .await
        .unwrap();
    assert_eq!(
        provider.stats(&key).await.unwrap().memory_bytes,
        Some(123456)
    );
    assert_eq!(
        provider
            .execute(&Command::DeleteContainer { key: key.clone() })
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    provider
        .lifecycle(&key, LifecycleAction::Stop)
        .await
        .unwrap();
    provider
        .execute(&Command::DeleteContainer { key: key.clone() })
        .await
        .unwrap();
    assert!(provider.snapshot().await.unwrap().containers.is_empty());
    provider.execute(&command).await.unwrap();
    assert_eq!(
        provider
            .execute(&Command::DeleteContainer { key })
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    let log = std::fs::read_to_string(temporary.path().join("arguments.jsonl")).unwrap();
    let calls: Vec<Vec<String>> = log
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert!(calls.iter().any(|args| args
        == &[
            "image",
            "pull",
            "--progress",
            "plain",
            "--",
            "example.invalid/contract-fixture:v1"
        ]));
    let creates: Vec<_> = calls
        .iter()
        .filter(|args| args[0] == "create" && !args.iter().any(|arg| arg == "--help"))
        .collect();
    assert_eq!(creates.len(), 2);
    assert!(
        creates
            .iter()
            .all(|args| args.contains(&"127.0.0.1:18080:80/tcp".into())
                && !args.contains(&"--rm".into()))
    );
    assert_eq!(
        calls
            .iter()
            .filter(|args| args[0] == "delete" && !args.iter().any(|arg| arg == "--help"))
            .count(),
        1
    );
    assert!(calls.iter().all(|args| !args.contains(&"--force".into())
        && !(args[0] == "delete" && args.contains(&"--all".into()))));
}

#[tokio::test]
async fn image_deletion_checks_aliases_stopped_users_retags_and_cleanup_failures() {
    let (temporary, provider) = fixture();
    let reference = "example.invalid/image:v1";
    let alias = "example.invalid/image:alias";
    for name in [reference, alias] {
        provider
            .execute(&Command::PullImage {
                environment: LOCAL_ENVIRONMENT.into(),
                reference: name.into(),
            })
            .await
            .unwrap();
    }
    let CommandResult::Container(container) = provider
        .execute(&Command::CreateContainer {
            environment: LOCAL_ENVIRONMENT.into(),
            spec: CreateContainer {
                options: Default::default(),
                name: "alias-user".into(),
                image: alias.into(),
                cpus: 1,
                memory_mib: 256,
                ports: vec![],
            },
        })
        .await
        .unwrap()
    else {
        panic!("container")
    };
    let image = provider
        .inspect_image(LOCAL_ENVIRONMENT, reference)
        .await
        .unwrap();
    assert_eq!(
        image.used_by.as_slice(),
        std::slice::from_ref(&container.key)
    );
    assert_eq!(image.other_references, [alias]);
    assert!(
        !serde_json::to_string(&image)
            .unwrap()
            .contains("fixture-secret")
    );
    let delete = Command::DeleteImage {
        key: image.key.clone(),
    };
    assert_eq!(
        provider.execute(&delete).await.unwrap_err().kind,
        ErrorKind::Conflict
    );
    provider
        .execute(&Command::DeleteContainer { key: container.key })
        .await
        .unwrap();

    let state_path = temporary.path().join("fixture-state.json");
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    state["digests"] = serde_json::json!({reference:format!("sha256:{}","c".repeat(64))});
    std::fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
    assert_eq!(
        provider.execute(&delete).await.unwrap_err().kind,
        ErrorKind::Conflict
    );
    let current = provider
        .inspect_image(LOCAL_ENVIRONMENT, reference)
        .await
        .unwrap();
    provider
        .execute(&Command::DeleteImage { key: current.key })
        .await
        .unwrap();
    assert_eq!(
        provider
            .resources(ResourceKind::Image)
            .await
            .unwrap()
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        [alias]
    );

    let current = provider
        .inspect_image(LOCAL_ENVIRONMENT, alias)
        .await
        .unwrap();
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    state["fail_cleanup"] = true.into();
    std::fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
    assert_eq!(
        provider
            .execute(&Command::DeleteImage { key: current.key })
            .await
            .unwrap_err()
            .kind,
        ErrorKind::UnknownOutcome
    );
    assert!(
        provider
            .resources(ResourceKind::Image)
            .await
            .unwrap()
            .is_empty()
    );
    let log = std::fs::read_to_string(temporary.path().join("arguments.jsonl")).unwrap();
    let deletes: Vec<Vec<String>> = log
        .lines()
        .map(|line| serde_json::from_str::<Vec<String>>(line).unwrap())
        .filter(|a| {
            a.starts_with(&["image".into(), "delete".into()])
                && !a.iter().any(|arg| arg == "--help")
        })
        .collect();
    assert_eq!(deletes.len(), 2);
    assert_eq!(deletes[0], ["image", "delete", "--", reference]);
    assert_eq!(deletes[1], ["image", "delete", "--", alias]);
}
