//! Read-only live verification. Prints counts only, never container log content.
use containerhub_apple::AppleProvider;
use containerhub_core::*;
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id = std::env::args()
        .nth(1)
        .ok_or("Pass a container ID for read-only log verification")?;
    let provider = AppleProvider::default();
    let snapshot = provider.snapshot().await?;
    let key = &snapshot
        .containers
        .iter()
        .find(|c| c.key.id == id)
        .ok_or("Container is missing")?
        .key;
    for source in [LogSource::Standard, LogSource::Boot] {
        let log = provider.logs(key, source).await?;
        assert_eq!(&log.key, key);
        assert_eq!(log.source, source);
        assert!(log.lines.len() <= logs::MAX_LINES);
        let bytes: usize = log.lines.iter().map(String::len).sum();
        assert!(bytes <= logs::MAX_BYTES);
        println!(
            "{source:?}: {} lines, {bytes} bytes, truncated={}",
            log.lines.len(),
            log.truncated
        );
    }
    let mut stale = key.clone();
    stale.incarnation.push_str("-stale");
    assert_eq!(
        provider
            .logs(&stale, LogSource::Standard)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    println!("PASS: read-only logs and stale identity rejection");
    Ok(())
}
