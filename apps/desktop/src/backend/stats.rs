use super::*;

pub(super) async fn worker(
    provider: Arc<dyn ManagementClient>,
    mut requests: watch::Receiver<Option<StatsRequest>>,
    events: Sender<Event>,
) {
    loop {
        let Some(request) = requests.borrow_and_update().clone() else {
            if requests.changed().await.is_err() {
                return;
            }
            continue;
        };
        let result = tokio::select! {
            biased;
            changed=requests.changed()=>{if changed.is_err(){return;}continue;}
            result=provider.stats(&request.key)=>result,
        };
        let failed = result.is_err();
        if events
            .send(Event::Stats(request.generation, result))
            .await
            .is_err()
        {
            return;
        }
        tokio::select! {changed=requests.changed()=>{if changed.is_err(){return;}},_=tokio::time::sleep(Duration::from_secs(if failed {10}else{2}))=>{}}
    }
}
