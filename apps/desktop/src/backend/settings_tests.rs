use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn cli_path_is_confirmed_before_saving_and_stale_appearance_changes_preserve_it() {
    let root = std::env::temp_dir().join(format!("containerhub-settings-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let binary = root.join("container");
    std::fs::write(
        &binary,
        include_bytes!("../../../../tests/fixtures/apple/cli_fixture.py"),
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    // A service failure must not prevent configuring a valid installed CLI.
    std::fs::write(
        root.join("fixture-state.json"),
        br#"{"images":[],"containers":[],"sequence":0,"service_offline":true}"#,
    )
    .unwrap();
    let (requests, receiver) = async_channel::bounded(8);
    let (events, results) = async_channel::bounded(8);
    requests
        .send(Request::ConfigureRuntime(Settings {
            apple_binary: Some(binary.clone()),
            ..Default::default()
        }))
        .await
        .unwrap();
    requests
        .send(Request::SaveSettings(Settings {
            appearance: Appearance::Dark,
            ..Default::default()
        }))
        .await
        .unwrap();
    requests
        .send(Request::ConfigureRuntime(Settings {
            apple_binary: Some(root.join("missing")),
            ..Default::default()
        }))
        .await
        .unwrap();
    drop(requests);
    discovery(
        Arc::new(containerhub_core::fake::FakeProvider::default()),
        &root,
        receiver,
        events,
    )
    .await;
    assert!(matches!(
        results.recv().await.unwrap(),
        Event::Snapshot(Ok(_))
    ));
    assert!(
        matches!(results.recv().await.unwrap(), Event::RuntimeConfigured(Ok(Some(path))) if path == binary)
    );
    assert!(matches!(
        results.recv().await.unwrap(),
        Event::RuntimeConfigured(Err(_))
    ));
    let settings: Settings = read_json(&root.join("settings.json")).unwrap();
    assert_eq!(settings.apple_binary, Some(binary));
    assert!(settings.appearance == Appearance::Dark);
    let old: Settings = serde_json::from_str(r#"{"appearance":"System"}"#).unwrap();
    assert!(old.apple_binary.is_none());
    assert_eq!(old.sort, ContainerSort::Name);
    std::fs::remove_dir_all(root).unwrap();
}
