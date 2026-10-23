use super::*;
use crate::contract_tests::fixture;

fn configure(root: &Path, extra: serde_json::Value) {
    let mut state = serde_json::json!({"images":[],"containers":[],"sequence":0});
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
fn calls(root: &Path) -> Vec<Vec<String>> {
    std::fs::read_to_string(root.join("arguments.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test]
async fn offline_diagnostics_keep_version_capabilities_and_cache_help_until_refresh() {
    let (temporary, provider) = fixture();
    configure(
        temporary.path(),
        serde_json::json!({"service_offline":true}),
    );
    let report = provider.diagnostics().await.unwrap();
    assert_eq!(report.cli_version.as_deref(), Some("1.6.0"));
    assert_eq!(
        report.cli_path,
        provider.binary.as_ref().map(|p| p.display().to_string())
    );
    assert!(report.service_error.is_some());
    assert!(
        report
            .checks
            .iter()
            .all(|check| check.available == (check.feature != RuntimeFeature::Stacks))
    );
    let before = calls(temporary.path()).len();
    provider
        .require(RuntimeFeature::CreateContainer)
        .await
        .unwrap();
    assert_eq!(
        calls(temporary.path()).len(),
        before,
        "cached help does not spawn CLI again"
    );
    assert!(
        provider.snapshot().await.is_err(),
        "offline is not an empty inventory"
    );
    provider.diagnostics().await.unwrap();
    assert_eq!(
        calls(temporary.path())
            .iter()
            .filter(|c| c.last().map(String::as_str) == Some("--help"))
            .count(),
        32
    );
}

#[tokio::test]
async fn missing_advanced_flag_and_lifecycle_command_block_only_their_contracts() {
    let (temporary, provider) = fixture();
    configure(
        temporary.path(),
        serde_json::json!({"missing_commands":["stop"],"missing_flags":{"create":["--mount"]}}),
    );
    let report = provider.diagnostics().await.unwrap();
    assert!(report.supports(RuntimeFeature::Containers));
    assert!(report.supports(RuntimeFeature::CreateContainer));
    assert!(!report.supports(RuntimeFeature::AdvancedCreate));
    assert!(!report.supports(RuntimeFeature::Lifecycle));
    let key = ResourceKey {
        environment: LOCAL_ENVIRONMENT.into(),
        provider: ProviderKind::Apple,
        namespace: None,
        id: "missing".into(),
        incarnation: "fixture".into(),
    };
    assert_eq!(
        provider
            .lifecycle(&key, LifecycleAction::Start)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    let spec = CreateContainer {
        name: "advanced".into(),
        image: "example.invalid/test:v1".into(),
        cpus: 1,
        memory_mib: 256,
        ports: vec![],
        options: CreateOptions {
            read_only: true,
            ..Default::default()
        },
    };
    assert_eq!(
        provider
            .execute(&Command::CreateContainer {
                environment: LOCAL_ENVIRONMENT.into(),
                spec
            })
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    assert!(
        calls(temporary.path())
            .iter()
            .all(|args| args.iter().any(|arg| arg == "--help")
                || args == &["--version"]
                || args[0] == "system")
    );
}

#[tokio::test]
async fn unknown_version_preserves_read_contracts_but_never_writes() {
    let (temporary, provider) = fixture();
    configure(temporary.path(), serde_json::json!({"version":"9.9.9"}));
    let report = provider.diagnostics().await.unwrap();
    for check in &report.checks {
        assert_eq!(
            check.available,
            !check.feature.writes() && check.feature != RuntimeFeature::Stacks
        );
        assert_eq!(check.reason.is_some(), !check.available);
    }
    assert!(provider.snapshot().await.unwrap().containers.is_empty());
    let command = Command::PullImage {
        environment: LOCAL_ENVIRONMENT.into(),
        reference: "example.invalid/test:v1".into(),
    };
    assert_eq!(
        provider.execute(&command).await.unwrap_err().kind,
        ErrorKind::Unsupported
    );
    assert!(
        !calls(temporary.path())
            .iter()
            .any(|a| a.starts_with(&["image".into(), "pull".into()])
                && !a.contains(&"--help".into()))
    );
}

#[tokio::test]
async fn explicit_missing_path_is_visible_and_never_falls_back_to_host_cli() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("missing-container");
    let provider = AppleProvider::with_binary(path.clone()).unwrap();
    let report = provider.diagnostics().await.unwrap();
    assert_eq!(report.cli_path.as_deref(), path.to_str());
    assert!(report.cli_version.is_none());
    assert_eq!(report.service_error.unwrap().kind, ErrorKind::NotInstalled);
    assert!(report.checks.iter().all(|check| !check.available));
    assert!(AppleProvider::with_binary("container".into()).is_err());
    assert!(validate_binary(temporary.path()).is_err());
}
