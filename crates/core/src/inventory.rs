use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerSort {
    #[default]
    Name,
    State,
    Image,
}
impl ContainerSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "名称",
            Self::State => "状态",
            Self::Image => "镜像",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::State,
            Self::State => Self::Image,
            Self::Image => Self::Name,
        }
    }
}
pub fn sort_containers(containers: &mut [Container], order: ContainerSort, descending: bool) {
    let rank = |c: &Container| match c.state {
        ContainerState::Running => 0,
        ContainerState::Stopped => 1,
        ContainerState::Unknown(_) => 2,
    };
    let provider_rank = |kind| match kind {
        ProviderKind::Apple => 0,
        ProviderKind::Docker => 1,
        ProviderKind::Kubernetes => 2,
    };
    containers.sort_by(|a, b| {
        let primary = match order {
            ContainerSort::Name => a.key.id.to_lowercase().cmp(&b.key.id.to_lowercase()),
            ContainerSort::State => rank(a).cmp(&rank(b)),
            ContainerSort::Image => a.image.to_lowercase().cmp(&b.image.to_lowercase()),
        };
        let primary = if descending {
            primary.reverse()
        } else {
            primary
        };
        primary
            .then_with(|| a.key.id.cmp(&b.key.id))
            .then_with(|| a.key.environment.cmp(&b.key.environment))
            .then_with(|| provider_rank(a.key.provider).cmp(&provider_rank(b.key.provider)))
            .then_with(|| a.key.namespace.cmp(&b.key.namespace))
            .then_with(|| a.key.incarnation.cmp(&b.key.incarnation))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(id: &str, state: ContainerState, image: &str) -> Container {
        Container {
            key: ResourceKey {
                environment: "test".into(),
                provider: ProviderKind::Apple,
                namespace: None,
                id: id.into(),
                incarnation: "first".into(),
            },
            image: image.into(),
            state,
            cpus: None,
            memory_limit: None,
            platform: String::new(),
            ports: vec![],
            mounts: vec![],
            networks: vec![],
            addresses: vec![],
            environment_names: vec![],
            label_names: vec![],
            created_at: String::new(),
            auto_remove: None,
            compose: None,
            health: crate::ContainerHealth::Unknown,
        }
    }
    #[test]
    fn sort_order_is_stable_across_refresh_and_full_identity_breaks_ties() {
        let mut rows = vec![
            row("c", ContainerState::Stopped, "A"),
            row("a", ContainerState::Running, "z"),
            row("b", ContainerState::Unknown("new".into()), "a"),
        ];
        sort_containers(&mut rows, ContainerSort::State, false);
        assert_eq!(
            rows.iter().map(|c| c.key.id.as_str()).collect::<Vec<_>>(),
            ["a", "c", "b"]
        );
        sort_containers(&mut rows, ContainerSort::Image, true);
        assert_eq!(
            rows.iter().map(|c| c.key.id.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        let mut duplicate = rows[0].clone();
        duplicate.key.provider = ProviderKind::Docker;
        rows.push(duplicate);
        sort_containers(&mut rows, ContainerSort::Name, true);
        let expected: Vec<_> = rows.iter().map(|c| c.key.clone()).collect();
        assert_eq!(
            expected.iter().map(|k| k.id.as_str()).collect::<Vec<_>>(),
            ["c", "b", "a", "a"]
        );
        assert_eq!(expected[2].provider, ProviderKind::Apple);
        rows.reverse();
        sort_containers(&mut rows, ContainerSort::Name, true);
        assert_eq!(
            rows.iter().map(|c| c.key.clone()).collect::<Vec<_>>(),
            expected
        );
    }
}
