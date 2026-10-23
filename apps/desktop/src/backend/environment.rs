//! Cancellable, selected-host reads. Container service failures do not hide OS telemetry.
use super::*;

#[derive(Clone)]
pub struct Watch {
    pub generation: u64,
    pub client: Arc<dyn ManagementClient>,
    pub interval: u64,
}
pub struct Sample {
    pub generation: u64,
    pub received_at: u64,
    pub request_ms: u128,
    pub host: Result<HostSnapshot, ManagementError>,
    pub containers: Result<Snapshot, ManagementError>,
    pub diagnostics: Result<RuntimeDiagnostics, ManagementError>,
}
async fn bounded<T>(
    future: impl std::future::Future<Output = Result<T, ManagementError>>,
) -> Result<T, ManagementError> {
    tokio::time::timeout(Duration::from_secs(12), future)
        .await
        .unwrap_or_else(|_| Err(ManagementError::new(ErrorKind::Unavailable, "连接超时")))
}
pub async fn worker(mut rx: watch::Receiver<Option<Watch>>, events: Sender<Event>) {
    loop {
        let Some(request) = rx.borrow_and_update().clone() else {
            if rx.changed().await.is_err() {
                break;
            }
            continue;
        };
        let read = async {
            tokio::join!(
                async {
                    let start = std::time::Instant::now();
                    let result = bounded(request.client.host_snapshot()).await;
                    (result, start.elapsed().as_millis())
                },
                bounded(request.client.snapshot()),
                bounded(request.client.diagnostics())
            )
        };
        let ((host, request_ms), containers, diagnostics) = tokio::select! {
            biased;
            changed = rx.changed() => { if changed.is_err() { break; } continue; }
            result = read => result,
        };
        let sample = Sample {
            generation: request.generation,
            received_at: now(),
            request_ms,
            host,
            containers,
            diagnostics,
        };
        if events
            .send(Event::Environment(Box::new(sample)))
            .await
            .is_err()
        {
            break;
        }
        tokio::select! {
            changed = rx.changed() => { if changed.is_err() { break; } }
            _ = tokio::time::sleep(Duration::from_secs(request.interval.clamp(2, 30))) => {}
        }
    }
}
