use super::*;
use crate::contract_tests::fixture;
use serde_json::json;
use std::path::Path;

fn configure(root: &Path, extra: serde_json::Value) {
    let mut state = json!({"images":[],"containers":[],"sequence":0,"service_offline":true});
    state
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    std::fs::write(
        root.join("fixture-state.json"),
        serde_json::to_vec(&state).unwrap(),
    )
    .unwrap();
}
fn starts(root: &Path) -> usize {
    std::fs::read_to_string(root.join("arguments.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Vec<String>>(line).unwrap())
        .filter(|args| {
            args.starts_with(&["system".into(), "start".into()]) && !args.contains(&"--help".into())
        })
        .count()
}
fn command() -> Command {
    Command::StartRuntime {
        environment: LOCAL_ENVIRONMENT.into(),
    }
}

#[tokio::test]
async fn offline_start_is_verified_and_concurrent_starts_are_idempotent() {
    let (root, provider) = fixture();
    configure(root.path(), json!({}));
    let cmd = command();
    let (a, b) = tokio::join!(provider.execute(&cmd), provider.execute(&cmd));
    assert!(matches!(a.unwrap(), CommandResult::Confirmed));
    assert!(matches!(b.unwrap(), CommandResult::Confirmed));
    assert_eq!(starts(root.path()), 1);
    assert!(
        provider
            .diagnostics()
            .await
            .unwrap()
            .service_error
            .is_none()
    );
    let state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path().join("fixture-state.json")).unwrap())
            .unwrap();
    assert_eq!(state["containers"], json!([]));
}

#[tokio::test]
async fn service_observation_decides_success_not_process_exit() {
    for (extra, expected) in [
        (json!({"service_start_exit_error":true}), None),
        (
            json!({"service_start_unconfirmed":true}),
            Some(ErrorKind::UnknownOutcome),
        ),
        (
            json!({"service_start_denied":true}),
            Some(ErrorKind::PermissionDenied),
        ),
    ] {
        let (root, provider) = fixture();
        configure(root.path(), extra);
        match (provider.execute(&command()).await, expected) {
            (Ok(CommandResult::Confirmed), None) => {}
            (Err(error), Some(kind)) => {
                assert_eq!(error.kind, kind);
                assert!(!error.message.contains("fixture-secret"));
            }
            pair => panic!("unexpected outcome: {pair:?}"),
        }
        assert_eq!(starts(root.path()), 1);
    }
}

#[tokio::test]
async fn missing_contract_unknown_version_invalid_status_and_wrong_environment_never_start() {
    for (extra, expected) in [
        (json!({"version":"9.9.9"}), ErrorKind::Unsupported),
        (
            json!({"missing_flags":{"system start":["--disable-kernel-install"]}}),
            ErrorKind::Unsupported,
        ),
        (
            json!({"service_status_invalid":true}),
            ErrorKind::InvalidData,
        ),
    ] {
        let (root, provider) = fixture();
        configure(root.path(), extra);
        assert_eq!(
            provider.execute(&command()).await.unwrap_err().kind,
            expected
        );
        assert_eq!(starts(root.path()), 0);
    }
    let (root, provider) = fixture();
    configure(root.path(), json!({}));
    assert_eq!(
        provider
            .execute(&Command::StartRuntime {
                environment: "wrong".into()
            })
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    assert!(!root.path().join("arguments.jsonl").exists());
}

#[tokio::test]
async fn startup_timeout_remains_uncertain_and_never_retries() {
    let (root, provider) = fixture();
    configure(root.path(), json!({"service_start_delay":2}));
    let error = provider
        .start_runtime_with_timeout(&ExecutionControl::default(), Duration::from_millis(80))
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::UnknownOutcome);
    assert_eq!(starts(root.path()), 1);
    assert!(
        provider
            .diagnostics()
            .await
            .unwrap()
            .service_error
            .is_some()
    );
}
