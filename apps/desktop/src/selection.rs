pub(crate) fn toggle_visible_selection<K: Clone + Eq + std::hash::Hash>(
    selected: &mut std::collections::HashSet<K>,
    visible: &[K],
) -> usize {
    if visible.iter().all(|key| selected.contains(key)) {
        for key in visible {
            selected.remove(key);
        }
        return 0;
    }
    let mut omitted = 0;
    for key in visible {
        if selected.contains(key) {
            continue;
        }
        if selected.len() < 32 {
            selected.insert(key.clone());
        } else {
            omitted += 1;
        }
    }
    omitted
}

#[cfg(test)]
mod tests {
    use super::toggle_visible_selection;
    use containerhub_core::{ProviderKind, ResourceKey};
    use std::collections::HashSet;
    fn key(id: usize) -> ResourceKey {
        ResourceKey {
            provider: ProviderKind::Apple,
            environment: "fixture".into(),
            namespace: None,
            id: id.to_string(),
            incarnation: "one".into(),
        }
    }
    #[test]
    fn filtered_selection_preserves_hidden_rows_and_caps_batch_size() {
        let rows: Vec<_> = (0..40).map(key).collect();
        let mut selected = HashSet::from([rows[39].clone()]);
        assert_eq!(toggle_visible_selection(&mut selected, &rows[..3]), 0);
        assert_eq!(selected.len(), 4);
        toggle_visible_selection(&mut selected, &rows[..3]);
        assert_eq!(selected, HashSet::from([rows[39].clone()]));
        assert_eq!(toggle_visible_selection(&mut selected, &rows[..35]), 4);
        assert_eq!(selected.len(), 32);
        assert!(selected.contains(&rows[39]));
    }
}
