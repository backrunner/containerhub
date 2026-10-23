use super::*;
use crate::contract_tests::fixture;
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct Checkpoint(Mutex<Vec<Container>>);
#[async_trait::async_trait]
impl CreationCheckpoint for Checkpoint {
    async fn save(&self, c: &Container) -> Result<(), ManagementError> {
        self.0.lock().unwrap().push(c.clone());
        Ok(())
    }
}
async fn original(provider: &AppleProvider) -> Container {
    let reference = "example.invalid/rebuild:v1";
    provider
        .execute(&Command::PullImage {
            environment: LOCAL_ENVIRONMENT.into(),
            reference: reference.into(),
        })
        .await
        .unwrap();
    let CommandResult::Container(container) = provider
        .execute(&Command::CreateContainer {
            environment: LOCAL_ENVIRONMENT.into(),
            spec: CreateContainer {
                name: "rebuild-fixture".into(),
                image: reference.into(),
                cpus: 2,
                memory_mib: 512,
                ports: vec![],
                options: CreateOptions {
                    networks: vec!["default".into()],
                    volumes: vec![VolumeMount {
                        name: "database-data".into(),
                        destination: "/data".into(),
                        read_only: false,
                    }],
                    ..Default::default()
                },
            },
        })
        .await
        .unwrap()
    else {
        panic!("container")
    };
    *container
}
fn command(c: &Container) -> Command {
    Command::RebuildPorts {
        key: c.key.clone(),
        expected_state: c.state.clone(),
        expected: c.ports.clone(),
        ports: vec![Port {
            host: "127.0.0.1".into(),
            host_port: 18080,
            container_port: 80,
            protocol: "tcp".into(),
        }],
    }
}
fn configure(path: &std::path::Path, change: impl FnOnce(&mut serde_json::Value)) {
    let file = path.join("fixture-state.json");
    let mut state = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    change(&mut state);
    std::fs::write(file, serde_json::to_vec(&state).unwrap()).unwrap();
}
fn calls(path: &std::path::Path) -> Vec<Vec<String>> {
    std::fs::read_to_string(path.join("arguments.jsonl"))
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
#[tokio::test]
async fn rebuild_preserves_configuration_changes_identity_and_removes_original() {
    for running in [false, true] {
        let (temporary, provider) = fixture();
        let mut old = original(&provider).await;
        if running {
            provider
                .lifecycle(&old.key, LifecycleAction::Start)
                .await
                .unwrap();
            old.state = ContainerState::Running;
        }
        let checkpoint = Arc::new(Checkpoint::default());
        let control = ExecutionControl::with_checkpoint(Default::default(), checkpoint.clone());
        let CommandResult::Container(new) = provider
            .execute_controlled(&command(&old), &control)
            .await
            .unwrap()
        else {
            panic!("replacement")
        };
        assert_ne!(old.key, new.key);
        assert_eq!(new.key.id, old.key.id);
        assert_eq!(new.state, old.state);
        assert_eq!(new.ports[0].host_port, 18080);
        assert_eq!(new.mounts, old.mounts);
        assert_eq!(new.cpus, old.cpus);
        let snapshot = provider.snapshot().await.unwrap();
        assert_eq!(snapshot.containers.len(), 1);
        assert_eq!(snapshot.containers[0].key, new.key);
        assert!(!checkpoint.0.lock().unwrap().is_empty());
        let calls = calls(temporary.path());
        let deletes: Vec<_> = calls
            .iter()
            .filter(|a| a[0] == "delete" && !a.contains(&"--help".into()))
            .collect();
        assert_eq!(deletes.len(), 1);
        assert_eq!(
            deletes[0],
            &vec!["delete".to_owned(), "--".into(), old.key.id.clone()]
        );
        let create = calls
            .iter()
            .rfind(|a| a[0] == "create" && !a.contains(&"--help".into()))
            .unwrap();
        assert!(create.contains(&"--env-file".into()));
        assert!(create.iter().any(|a| a.contains("@sha256:")));
        assert!(
            !create.iter().any(|a| a.contains("TOKEN=fixture-secret")),
            "environment values must only use stdin"
        );
    }
}
#[tokio::test]
async fn rejects_unsupported_configuration_and_stale_ports_before_stopping() {
    let (temporary, provider) = fixture();
    let old = original(&provider).await;
    configure(temporary.path(), |s| {
        s["containers"][0]["configuration"]["futureFlag"] = true.into()
    });
    let control =
        ExecutionControl::with_checkpoint(Default::default(), Arc::new(Checkpoint::default()));
    assert_eq!(
        provider
            .execute_controlled(&command(&old), &control)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    assert!(
        !calls(temporary.path())
            .iter()
            .any(|a| ["stop", "delete"].contains(&a[0].as_str()) && !a.contains(&"--help".into()))
    );
    configure(temporary.path(), |s| {
        s["containers"][0]["configuration"]
            .as_object_mut()
            .unwrap()
            .remove("futureFlag");
    });
    let mut stale = command(&old);
    if let Command::RebuildPorts { expected, .. } = &mut stale {
        expected.push(Port {
            host: "127.0.0.1".into(),
            host_port: 9999,
            container_port: 80,
            protocol: "tcp".into(),
        });
    }
    assert_eq!(
        provider
            .execute_controlled(&stale, &control)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    assert_eq!(
        provider.snapshot().await.unwrap().containers[0].key,
        old.key
    );
}
#[tokio::test]
async fn create_failure_after_delete_is_unknown_and_replay_cannot_delete_again() {
    let (temporary, provider) = fixture();
    let old = original(&provider).await;
    configure(temporary.path(), |s| {
        s["fail_create_after_delete"] = true.into()
    });
    let control =
        ExecutionControl::with_checkpoint(Default::default(), Arc::new(Checkpoint::default()));
    let command = command(&old);
    assert_eq!(
        provider
            .execute_controlled(&command, &control)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::UnknownOutcome
    );
    assert!(control.created().is_none());
    assert!(provider.snapshot().await.unwrap().containers.is_empty());
    assert!(
        provider
            .execute_controlled(&command, &control)
            .await
            .is_err()
    );
    assert_eq!(
        calls(temporary.path())
            .iter()
            .filter(|a| a[0] == "delete" && !a.contains(&"--help".into()))
            .count(),
        1
    );
}
#[tokio::test]
async fn failed_start_keeps_verified_replacement_and_retry_is_start_only() {
    let (temporary, provider) = fixture();
    let mut old = original(&provider).await;
    provider
        .lifecycle(&old.key, LifecycleAction::Start)
        .await
        .unwrap();
    old.state = ContainerState::Running;
    configure(temporary.path(), |s| s["fail_start"] = true.into());
    let control =
        ExecutionControl::with_checkpoint(Default::default(), Arc::new(Checkpoint::default()));
    let command = command(&old);
    assert_eq!(
        provider
            .execute_controlled(&command, &control)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Unavailable
    );
    let created = control.created().unwrap();
    assert_ne!(created.key, old.key);
    let snapshot = provider.snapshot().await.unwrap();
    let retry = start_created_command(
        &command,
        Some(&CommandResult::Container(Box::new(created.clone()))),
        &snapshot,
    )
    .unwrap();
    assert_eq!(
        retry,
        Command::Lifecycle {
            key: created.key,
            action: LifecycleAction::Start
        }
    );
}
#[test]
fn captured_configuration_is_accepted_but_unknown_mounts_are_not() {
    let bytes = include_bytes!("../../../../../tests/fixtures/apple/1.6.0/containers.json");
    let c = parse_containers(bytes).unwrap().remove(0);
    let plan = plan::Plan::parse(bytes, &c, &[]).unwrap();
    assert!(plan.args.contains(&"default,mtu=1280".into()));
    assert!(!plan.args.iter().any(|a| a.contains("REDACTED")));
    let mut data: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    data[0]["configuration"]["mounts"] = serde_json::json!([{"source":"/tmp/x","destination":"/data","options":[],"type":{"block":{"format":"ext4"}}}]);
    assert!(plan::Plan::parse(&serde_json::to_vec(&data).unwrap(), &c, &[]).is_err());
}
