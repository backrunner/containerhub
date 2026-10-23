use super::*;
use serde::Deserialize;
use std::{collections::HashMap, time::Instant};

pub(super) type Samples = HashMap<ResourceKey, (Instant, u64)>;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sample {
    id: String,
    memory_usage_bytes: Option<u64>,
    memory_limit_bytes: Option<u64>,
    cpu_usage_usec: Option<u64>,
}
impl AppleProvider {
    pub(super) async fn read_stats(
        &self,
        key: &ResourceKey,
    ) -> Result<ContainerStats, ManagementError> {
        if self.version().await? != "1.6.0" {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "当前版本未验证统计格式",
            ));
        }
        if self.current(key).await?.state != ContainerState::Running {
            return Err(ManagementError::new(
                ErrorKind::Unavailable,
                "容器未运行，暂无实时采样",
            ));
        }
        let bytes = self
            .run(
                &["stats", "--format", "json", "--no-stream", "--", &key.id],
                false,
            )
            .await?;
        self.current(key).await?;
        let mut samples = self
            .samples
            .lock()
            .map_err(|_| ManagementError::new(ErrorKind::Unavailable, "采样缓存不可用"))?;
        parse_sample(&bytes, key, &mut samples, Instant::now())
    }
}
fn parse_sample(
    bytes: &[u8],
    key: &ResourceKey,
    samples: &mut Samples,
    time: Instant,
) -> Result<ContainerStats, ManagementError> {
    let mut rows: Vec<Sample> = serde_json::from_slice(bytes)
        .map_err(|_| ManagementError::new(ErrorKind::InvalidData, "统计 JSON 格式不兼容"))?;
    if rows.len() != 1 || rows[0].id != key.id {
        return Err(ManagementError::new(
            ErrorKind::Unavailable,
            "运行时未返回目标容器的有效采样",
        ));
    }
    let row = rows.remove(0);
    samples.retain(|_, (at, _)| time.duration_since(*at).as_secs() < 60);
    if samples.len() >= 256 {
        samples.clear();
    }
    let cpu_percent = row.cpu_usage_usec.and_then(|usage| {
        let previous = samples.insert(key.clone(), (time, usage));
        previous.and_then(|(at, old)| {
            let seconds = time.duration_since(at).as_secs_f64();
            (usage >= old && seconds >= 0.1)
                .then(|| (usage - old) as f64 / 1_000_000.0 / seconds * 100.0)
        })
    });
    if row.cpu_usage_usec.is_none() {
        samples.remove(key);
    }
    Ok(ContainerStats {
        key: key.clone(),
        observed_at: now(),
        cpu_percent,
        memory_bytes: row.memory_usage_bytes,
        memory_limit_bytes: row.memory_limit_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_uses_valid_interval_and_rejects_missing_or_recreated_targets() {
        let key = ResourceKey {
            environment: LOCAL_ENVIRONMENT.into(),
            provider: ProviderKind::Apple,
            namespace: None,
            id: "stats-test".into(),
            incarnation: "one".into(),
        };
        let mut cache = Samples::new();
        let time = Instant::now();
        let sample = |usage| {
            serde_json::to_vec(&serde_json::json!([{"id":"stats-test","cpuUsageUsec":usage,"memoryUsageBytes":1234,"memoryLimitBytes":536870912}])).unwrap()
        };
        assert!(
            parse_sample(&sample(100), &key, &mut cache, time)
                .unwrap()
                .cpu_percent
                .is_none()
        );
        let measured = parse_sample(
            &sample(1_000_100),
            &key,
            &mut cache,
            time + Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(measured.cpu_percent, Some(50.0));
        assert_eq!(measured.memory_bytes, Some(1234));
        assert!(
            parse_sample(&sample(50), &key, &mut cache, time + Duration::from_secs(4))
                .unwrap()
                .cpu_percent
                .is_none()
        );
        let mut replacement = key.clone();
        replacement.incarnation = "two".into();
        assert!(
            parse_sample(
                &sample(500),
                &replacement,
                &mut cache,
                time + Duration::from_secs(5)
            )
            .unwrap()
            .cpu_percent
            .is_none()
        );
        assert!(parse_sample(b"[]", &key, &mut cache, time).is_err());
        assert!(parse_sample(br#"[{"id":"other"}]"#, &key, &mut cache, time).is_err());
    }
}
