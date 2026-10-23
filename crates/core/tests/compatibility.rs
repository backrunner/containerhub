use containerhub_core::*;

#[test]
fn legacy_journals_migrate_without_losing_pending_identity_or_accepting_ambiguous_commands() {
    let mut record = serde_json::json!({"id":42,"key":{"environment":"apple:local:current-user","id":"original","incarnation":"creation|digest"},"action":"Restart","state":"Running","started_at":123,"finished_at":null,"message":null});
    let operation: Operation = serde_json::from_value(record.clone()).unwrap();
    assert_eq!(
        operation.command.resource_key().unwrap().provider,
        ProviderKind::Apple
    );
    assert!(matches!(
        operation.command,
        Command::Lifecycle {
            action: LifecycleAction::Restart,
            ..
        }
    ));
    assert_eq!(operation.state, OperationState::Running);
    assert!(
        operation.receipt.is_none(),
        "old journals must not invent a remote request ID"
    );
    let migrated = serde_json::to_value(&operation).unwrap();
    assert!(migrated.get("key").is_none());
    assert!(migrated.get("command").is_some());
    record["command"] = migrated["command"].clone();
    assert!(serde_json::from_value::<Operation>(record).is_err());
}

#[test]
fn new_commands_reject_flag_injection_invalid_ports_and_resource_overcommit() {
    for reference in [
        "--all",
        "https://registry/image",
        "image;command",
        "image\nother",
        "",
    ] {
        assert!(
            Command::PullImage {
                environment: "demo".into(),
                reference: reference.into()
            }
            .validate()
            .is_err()
        );
    }
    let mut spec = CreateContainer {
        options: Default::default(),
        name: "valid-name".into(),
        image: "registry.example:5000/team/image@sha256:abcd".into(),
        cpus: 2,
        memory_mib: 512,
        ports: vec![],
    };
    let valid = |spec| Command::CreateContainer {
        environment: "demo".into(),
        spec,
    };
    assert!(valid(spec.clone()).validate().is_ok());
    spec.ports.push(Port {
        host: "127.0.0.1".into(),
        host_port: 8080,
        container_port: 80,
        protocol: "tcp".into(),
    });
    assert!(valid(spec.clone()).validate().is_ok());
    spec.ports.push(spec.ports[0].clone());
    assert!(valid(spec.clone()).validate().is_err());
    spec.ports.clear();
    spec.cpus = 0;
    assert!(valid(spec.clone()).validate().is_err());
    spec.cpus = 2;
    spec.memory_mib = u32::MAX;
    assert!(valid(spec).validate().is_err());
}
