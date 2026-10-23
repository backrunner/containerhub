use super::*;

/// An actual container, with optional inspection from the connected Docker node.
/// Remote nodes expose identity only; task state is deliberately not container state.
pub struct StackContainerOverview<'a> {
    pub id: &'a str,
    pub node: Option<&'a str>,
    pub inspected: Option<&'a Container>,
}

impl StackService {
    pub fn container_overview(&self) -> Vec<StackContainerOverview<'_>> {
        let Some(service) = &self.swarm else {
            return self
                .containers
                .iter()
                .map(|c| StackContainerOverview {
                    id: &c.key.incarnation,
                    node: None,
                    inspected: Some(c),
                })
                .collect();
        };
        let mut seen = BTreeSet::new();
        let mut tasks: Vec<_> = service
            .tasks
            .iter()
            .filter(|t| !matches!(t.desired_state.as_str(), "shutdown" | "remove"))
            .collect();
        tasks.sort_by_key(|t| (t.slot, &t.node, &t.id));
        tasks
            .into_iter()
            .filter_map(|t| {
                let id = t
                    .container_id
                    .as_deref()
                    .filter(|id| id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()))?;
                if !seen.insert(id) {
                    return None;
                }
                Some(StackContainerOverview {
                    id,
                    node: (!t.node.is_empty()).then_some(t.node.as_str()),
                    inspected: self.containers.iter().find(|c| {
                        c.key.environment == service.key.environment
                            && c.key.provider == ProviderKind::Docker
                            && c.key.incarnation == id
                    }),
                })
            })
            .collect()
    }
}
