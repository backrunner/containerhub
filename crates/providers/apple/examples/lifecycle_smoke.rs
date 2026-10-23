use containerhub_apple::AppleProvider;
use containerhub_core::*;
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id = std::env::args()
        .nth(1)
        .ok_or("Pass the ID of a disposable containerhub-smoke-* container")?;
    if !id.starts_with("containerhub-smoke-") {
        return Err("Only dedicated smoke containers are accepted".into());
    }
    let provider = AppleProvider::default();
    let before = provider.snapshot().await?;
    let key = before
        .containers
        .iter()
        .find(|c| c.key.id == id)
        .ok_or("Smoke container is missing")?
        .key
        .clone();
    println!(
        "runtime {}: {} containers",
        before.cli_version,
        before.containers.len()
    );
    for kind in [
        ResourceKind::Image,
        ResourceKind::Volume,
        ResourceKind::Network,
    ] {
        println!(
            "{}: {} resources",
            kind.label(),
            provider.resources(kind).await?.len()
        );
    }
    let mut stale = key.clone();
    stale.incarnation.push_str("-stale");
    assert_eq!(
        provider
            .lifecycle(&stale, LifecycleAction::Start)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    println!("stale identity rejected");
    for action in [
        LifecycleAction::Start,
        LifecycleAction::Restart,
        LifecycleAction::Stop,
    ] {
        provider.lifecycle(&key, action).await?;
        let actual = provider
            .snapshot()
            .await?
            .containers
            .into_iter()
            .find(|c| c.key == key)
            .ok_or("Identity changed")?;
        assert_eq!(actual.state.is_running(), action != LifecycleAction::Stop);
        println!("{}: {}", action.label(), actual.state.label());
    }
    Ok(())
}
