//! Iterative strongly connected components, ordered with prerequisites first.
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

/// Groups cycles without treating them as semantic validity. The caller retains cycle rules.
pub(crate) fn dependency_groups<K: Clone + Eq + Hash>(
    starts: &[K],
    dependencies: &HashMap<K, Vec<K>>,
) -> Vec<Vec<K>> {
    let mut seen = HashSet::new();
    let mut finish = Vec::new();
    for root in starts {
        let mut stack = vec![(root.clone(), false)];
        while let Some((node, leaving)) = stack.pop() {
            if leaving {
                finish.push(node);
                continue;
            }
            if !seen.insert(node.clone()) {
                continue;
            }
            stack.push((node.clone(), true));
            stack.extend(
                dependencies
                    .get(&node)
                    .into_iter()
                    .flatten()
                    .rev()
                    .cloned()
                    .map(|next| (next, false)),
            );
        }
    }
    let mut reverse: HashMap<K, Vec<K>> = HashMap::new();
    for (source, targets) in dependencies {
        for target in targets {
            reverse.entry(target.clone()).or_default().push(source.clone());
        }
    }
    // Sort reverse adjacency by discovery order for stable traversal independent of HashMap order.
    let order: HashMap<_, _> = finish.iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();
    for incoming in reverse.values_mut() {
        incoming.sort_by_key(|id| order.get(id).copied());
    }
    seen.clear();
    let mut groups = Vec::new();
    for root in finish.into_iter().rev() {
        if seen.contains(&root) {
            continue;
        }
        let mut group = Vec::new();
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if !seen.insert(node.clone()) {
                continue;
            }
            pending.extend(reverse.get(&node).into_iter().flatten().cloned());
            group.push(node);
        }
        groups.push(group);
    }
    groups.reverse();
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutual_definitions_precede_consumers_and_follow_external_dependencies() {
        let graph = HashMap::from([(0, vec![1]), (1, vec![2]), (2, vec![1, 3]), (3, vec![])]);
        let groups = dependency_groups(&[0], &graph);
        assert_eq!(groups.first(), Some(&vec![3]));
        assert_eq!(groups.last(), Some(&vec![0]));
        assert_eq!(groups[1].iter().copied().collect::<HashSet<_>>(), HashSet::from([1, 2]));
    }
    #[test]
    fn self_description_and_disconnected_roots_terminate() {
        let graph = HashMap::from([(0, vec![0]), (1, vec![])]);
        let groups = dependency_groups(&[0, 1], &graph);
        assert_eq!(groups.len(), 2);
        assert!(groups.contains(&vec![0]));
        assert!(groups.contains(&vec![1]));
    }
}
