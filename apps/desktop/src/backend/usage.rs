use super::*;
use futures_util::{StreamExt, stream};

pub(super) async fn worker(
    provider: Arc<dyn ManagementClient>,
    mut requests: watch::Receiver<Option<UsageRequest>>,
    events: Sender<Event>,
) {
    loop {
        let Some(request) = requests.borrow_and_update().clone() else {
            if requests.changed().await.is_err() {
                return;
            }
            continue;
        };
        let samples = stream::iter(request.keys.iter().cloned().map(|key| {
            let provider = provider.clone();
            async move {
                let result = tokio::time::timeout(Duration::from_secs(6), provider.stats(&key))
                    .await
                    .unwrap_or_else(|_| {
                        Err(ManagementError::new(ErrorKind::Timeout, "资源采样超时"))
                    });
                (key, result)
            }
        }))
        .buffer_unordered(4)
        .collect::<Vec<_>>();
        let results = tokio::select! {
            biased;
            changed = requests.changed() => { if changed.is_err() { return; } continue; }
            results = samples => results,
        };
        if events
            .send(Event::Usage(request.generation, results))
            .await
            .is_err()
        {
            return;
        }
        tokio::select! {
            changed = requests.changed() => { if changed.is_err() { return; } }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
    }
}
