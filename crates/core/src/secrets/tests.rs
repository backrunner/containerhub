use super::*;
#[test]
fn values_are_literal_bounded_redacted_and_bound_to_declared_names() {
    let mut input = SecretEnvironment::default();
    input
        .insert("API_TOKEN".into(), "  canary=$HOME # '中文'=  ".into())
        .unwrap();
    input.insert("EMPTY".into(), "".into()).unwrap();
    assert_eq!(
        &**input.stdin_bytes().unwrap(),
        "API_TOKEN=  canary=$HOME # '中文'=  \nEMPTY=\n".as_bytes()
    );
    assert!(!format!("{input:?}").contains("canary"));
    assert!(input.insert("API_TOKEN".into(), "changed".into()).is_err());
    for bad in ["1TOKEN", "X=Y", "X\nY", "", "bad-name"] {
        assert!(input.insert(bad.into(), "canary".into()).is_err());
    }
    for c in [
        '\0', '\r', '\n', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}',
    ] {
        assert!(
            input
                .insert("BAD".into(), format!("canary{c}OTHER=injected"))
                .is_err()
        );
    }
    assert!(
        input
            .insert("BIG".into(), "x".repeat(16 * 1024 + 1))
            .is_err()
    );
    let mut command = crate::Command::CreateContainer {
        environment: "demo".into(),
        spec: crate::CreateContainer {
            name: "test".into(),
            image: "image:v1".into(),
            cpus: 1,
            memory_mib: 256,
            ports: vec![],
            options: crate::CreateOptions {
                secret_environment: input.names(),
                ..Default::default()
            },
        },
    };
    input.validate_for(&command).unwrap();
    assert!(!serde_json::to_string(&command).unwrap().contains("canary"));
    assert!(SecretEnvironment::default().validate_for(&command).is_err());
    if let Command::CreateContainer { spec, .. } = &mut command {
        spec.options.secret_environment.push("MISSING".into());
    }
    assert!(input.validate_for(&command).is_err());
}
#[test]
fn input_limits_also_apply_to_transport_deserialization() {
    let mut values = serde_json::Map::new();
    for i in 0..33 {
        values.insert(format!("KEY_{i}"), "x".into());
    }
    let input: SecretEnvironment = serde_json::from_value(values.into()).unwrap();
    assert!(input.validate().is_err());
    let input: SecretEnvironment = serde_json::from_value(serde_json::json!({"A":"x".repeat(16384),"B":"x".repeat(16384),"C":"x".repeat(16384),"D":"x".repeat(16384)})).unwrap();
    assert!(input.validate().is_err());
}
