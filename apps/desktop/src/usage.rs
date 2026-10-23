use crate::*;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct UsageView {
    pub generation: u64,
    pub keys: Vec<ResourceKey>,
    pub samples: HashMap<ResourceKey, ContainerStats>,
}
impl UsageView {
    pub fn sample(&self, key: &ResourceKey) -> Option<&ContainerStats> {
        self.samples
            .get(key)
            .filter(|sample| now().saturating_sub(sample.observed_at) <= 15)
    }
    pub fn totals(&self) -> (Option<f64>, Option<u64>) {
        if self.keys.is_empty() {
            return (Some(0.), Some(0));
        }
        let cpu = self
            .keys
            .iter()
            .map(|key| {
                self.sample(key)?
                    .cpu_percent
                    .filter(|v| v.is_finite() && *v >= 0.)
            })
            .sum();
        let memory = self
            .keys
            .iter()
            .map(|key| self.sample(key)?.memory_bytes)
            .sum();
        (cpu, memory)
    }
}
impl Workspace {
    pub(super) fn request_usage(&mut self) {
        let mut keys: Vec<_> = self
            .snapshot
            .as_ref()
            .filter(|s| {
                s.capabilities.stats
                    && self.is_active
                    && matches!(self.page, Page::Containers | Page::Stacks)
            })
            .map(|s| {
                s.containers
                    .iter()
                    .filter(|c| c.state.is_running())
                    .map(|c| c.key.clone())
                    .collect()
            })
            .unwrap_or_default();
        keys.sort_by(|a, b| {
            (&a.environment, &a.id, &a.incarnation).cmp(&(&b.environment, &b.id, &b.incarnation))
        });
        if self.usage.keys == keys {
            return;
        }
        self.usage.generation += 1;
        self.usage.samples.retain(|key, _| keys.contains(key));
        self.usage.keys = keys.clone();
        self.backend
            .usage
            .send_replace((!keys.is_empty()).then_some(backend::UsageRequest {
                generation: self.usage.generation,
                keys,
            }));
    }
}

#[cfg(test)]
mod tests {
    use super::UsageView;
    use containerhub_core::{ContainerStats, ProviderKind, ResourceKey, now};
    #[test]
    fn totals_never_report_partial_or_stale_samples_as_complete_usage() {
        let key = |id: &str| ResourceKey {
            provider: ProviderKind::Apple,
            environment: "fixture".into(),
            namespace: None,
            id: id.into(),
            incarnation: "one".into(),
        };
        let mut usage = UsageView {
            keys: vec![key("a"), key("b")],
            ..Default::default()
        };
        let sample = |key| ContainerStats {
            key,
            observed_at: now(),
            cpu_percent: Some(2.),
            memory_bytes: Some(100),
            memory_limit_bytes: None,
        };
        usage.samples.insert(key("a"), sample(key("a")));
        assert_eq!(usage.totals(), (None, None));
        usage.samples.insert(key("b"), sample(key("b")));
        assert_eq!(usage.totals(), (Some(4.), Some(200)));
        usage.samples.get_mut(&key("b")).unwrap().cpu_percent = None;
        assert_eq!(usage.totals(), (None, Some(200)));
        usage.samples.get_mut(&key("b")).unwrap().observed_at = now() - 16;
        assert_eq!(usage.totals(), (None, None));
    }
}
