use super::*;
use futures_util::{StreamExt, stream};

pub type Results = Vec<(String, Result<ImageDetails, ManagementError>)>;

pub(super) async fn inspect(
    provider: Arc<dyn ManagementClient>,
    environment: String,
    references: Vec<String>,
) -> Results {
    stream::iter(references.into_iter().take(32))
        .map(|reference| {
            let provider = provider.clone();
            let environment = environment.clone();
            async move {
                let result = tokio::time::timeout(
                    Duration::from_secs(10),
                    provider.inspect_image(&environment, &reference),
                )
                .await
                .unwrap_or_else(|_| {
                    Err(ManagementError::new(
                        ErrorKind::Unavailable,
                        "读取超时，请重试",
                    ))
                });
                (reference, result)
            }
        })
        .buffered(4)
        .collect()
        .await
}
