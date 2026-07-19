use std::{collections::HashSet, hash::Hash};

use crate::graph::{DescriptorGraph, DescriptorSemanticsError};

/// Representation-neutral state machine for a self-first `Extends` traversal.
///
/// The state machine prepares the next edge after yielding the current node. Structural failures
/// are therefore emitted on the following iteration, preserving the runtime API's ability to stop
/// before an error farther up the chain.
pub struct ExtendsTraversal<N, I, E> {
    next: Option<N>,
    pending_error: Option<DescriptorSemanticsError<E, N>>,
    visited: HashSet<I>,
    finished: bool,
}

impl<N, I, E> ExtendsTraversal<N, I, E>
where
    N: Clone,
    I: Clone + Eq + Hash,
{
    pub fn new(start: N) -> Self {
        Self { next: Some(start), pending_error: None, visited: HashSet::new(), finished: false }
    }

    pub fn next_with(
        &mut self,
        identity: impl Fn(&N) -> I,
        extends_targets: impl FnOnce(&N) -> Result<Vec<N>, E>,
    ) -> Option<Result<N, DescriptorSemanticsError<E, N>>> {
        if self.finished {
            return None;
        }

        if let Some(error) = self.pending_error.take() {
            self.finished = true;
            return Some(Err(error));
        }

        let current = self.next.take()?;
        self.visited.insert(identity(&current));

        match extends_targets(&current) {
            Ok(targets) => match targets.as_slice() {
                [] => self.finished = true,
                [parent] => {
                    if self.visited.contains(&identity(parent)) {
                        self.pending_error = Some(DescriptorSemanticsError::CyclicExtends {
                            descriptor: parent.clone(),
                        });
                    } else {
                        self.next = Some(parent.clone());
                    }
                }
                many => {
                    self.pending_error = Some(DescriptorSemanticsError::MultipleExtends {
                        descriptor: current.clone(),
                        count: many.len(),
                    });
                }
            },
            Err(error) => self.pending_error = Some(DescriptorSemanticsError::Access(error)),
        }

        Some(Ok(current))
    }
}

/// Lazy, self-first `Extends` iterator over a descriptor graph.
pub struct ExtendsWalk<'a, G: DescriptorGraph> {
    graph: &'a G,
    traversal: ExtendsTraversal<G::Node, G::Identity, G::Error>,
}

impl<'a, G: DescriptorGraph> Iterator for ExtendsWalk<'a, G> {
    type Item = Result<G::Node, DescriptorSemanticsError<G::Error, G::Node>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.traversal
            .next_with(|node| self.graph.identity(node), |node| self.graph.extends_targets(node))
    }
}

pub fn walk_extends_chain<'a, G: DescriptorGraph>(
    graph: &'a G,
    start: &G::Node,
) -> ExtendsWalk<'a, G> {
    ExtendsWalk { graph, traversal: ExtendsTraversal::new(start.clone()) }
}

pub fn ancestors<G: DescriptorGraph>(
    graph: &G,
    start: &G::Node,
) -> Result<Vec<G::Node>, DescriptorSemanticsError<G::Error, G::Node>> {
    walk_extends_chain(graph, start).collect()
}

pub fn equals_or_extends<G: DescriptorGraph>(
    graph: &G,
    candidate: &G::Node,
    anchor: &G::Node,
) -> Result<bool, DescriptorSemanticsError<G::Error, G::Node>> {
    let anchor_id = graph.identity(anchor);
    for ancestor in walk_extends_chain(graph, candidate) {
        if graph.identity(&ancestor?) == anchor_id {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Computes the effective descriptor lineage defined by MAP Type System v1.2.
pub fn effective_descriptor_lineage<G: DescriptorGraph>(
    graph: &G,
    holon: &G::Node,
) -> Result<Vec<G::Node>, DescriptorSemanticsError<G::Error, G::Node>> {
    let described_by_targets =
        graph.described_by_targets(holon).map_err(DescriptorSemanticsError::Access)?;
    let described_by = match described_by_targets.as_slice() {
        [] => return ancestors(graph, holon),
        [descriptor] => descriptor,
        many => {
            return Err(DescriptorSemanticsError::MultipleDescribedBy {
                holon: holon.clone(),
                count: many.len(),
            })
        }
    };

    if graph.is_type_descriptor(described_by).map_err(DescriptorSemanticsError::Access)? {
        let mut lineage = ancestors(graph, holon)?;
        append_unique(graph, &mut lineage, ancestors(graph, described_by)?);
        return Ok(lineage);
    }

    ancestors(graph, described_by)
}

/// Collects related members in self-first lineage order, deduplicated by node identity.
pub fn flatten_related_members<G: DescriptorGraph>(
    graph: &G,
    start: &G::Node,
    member_kind: &G::MemberKind,
) -> Result<Vec<G::Node>, DescriptorSemanticsError<G::Error, G::Node>> {
    let mut members = Vec::new();
    let mut seen = HashSet::new();

    for ancestor in walk_extends_chain(graph, start) {
        let ancestor = ancestor?;
        for member in graph
            .related_members(&ancestor, member_kind)
            .map_err(DescriptorSemanticsError::Access)?
        {
            if seen.insert(graph.identity(&member)) {
                members.push(member);
            }
        }
    }

    Ok(members)
}

fn append_unique<G: DescriptorGraph>(
    graph: &G,
    lineage: &mut Vec<G::Node>,
    additional: Vec<G::Node>,
) {
    let mut seen = lineage.iter().map(|node| graph.identity(node)).collect::<HashSet<_>>();
    for descriptor in additional {
        if seen.insert(graph.identity(&descriptor)) {
            lineage.push(descriptor);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;

    #[derive(Default)]
    struct TestGraph {
        extends: HashMap<&'static str, Vec<&'static str>>,
        described_by: HashMap<&'static str, Vec<&'static str>>,
        members: HashMap<(&'static str, &'static str), Vec<&'static str>>,
        type_descriptors: HashSet<&'static str>,
    }

    impl DescriptorGraph for TestGraph {
        type Node = &'static str;
        type Identity = &'static str;
        type MemberKind = &'static str;
        type Error = &'static str;

        fn identity(&self, node: &Self::Node) -> Self::Identity {
            node
        }

        fn extends_targets(&self, node: &Self::Node) -> Result<Vec<Self::Node>, Self::Error> {
            Ok(self.extends.get(node).cloned().unwrap_or_default())
        }

        fn described_by_targets(&self, node: &Self::Node) -> Result<Vec<Self::Node>, Self::Error> {
            Ok(self.described_by.get(node).cloned().unwrap_or_default())
        }

        fn related_members(
            &self,
            node: &Self::Node,
            member_kind: &Self::MemberKind,
        ) -> Result<Vec<Self::Node>, Self::Error> {
            Ok(self.members.get(&(*node, *member_kind)).cloned().unwrap_or_default())
        }

        fn is_type_descriptor(&self, node: &Self::Node) -> Result<bool, Self::Error> {
            Ok(self.type_descriptors.contains(node))
        }
    }

    #[test]
    fn ancestors_are_self_first_and_identity_based() {
        let graph = TestGraph {
            extends: HashMap::from([("C", vec!["B"]), ("B", vec!["A"])]),
            ..Default::default()
        };

        assert_eq!(ancestors(&graph, &"C"), Ok(vec!["C", "B", "A"]));
        assert_eq!(equals_or_extends(&graph, &"C", &"A"), Ok(true));
        assert_eq!(equals_or_extends(&graph, &"C", &"Other"), Ok(false));
    }

    #[test]
    fn traversal_defers_structural_errors_until_after_the_current_node() {
        let graph =
            TestGraph { extends: HashMap::from([("Child", vec!["A", "B"])]), ..Default::default() };
        let mut walk = walk_extends_chain(&graph, &"Child");

        assert_eq!(walk.next(), Some(Ok("Child")));
        assert_eq!(
            walk.next(),
            Some(Err(DescriptorSemanticsError::MultipleExtends { descriptor: "Child", count: 2 }))
        );
        assert_eq!(walk.next(), None);
    }

    #[test]
    fn cycles_are_reported_at_the_repeated_identity() {
        let graph = TestGraph {
            extends: HashMap::from([("A", vec!["B"]), ("B", vec!["A"])]),
            ..Default::default()
        };

        assert_eq!(
            ancestors(&graph, &"A"),
            Err(DescriptorSemanticsError::CyclicExtends { descriptor: "A" })
        );
    }

    #[test]
    fn ordinary_holons_use_their_describing_descriptor_lineage() {
        let graph = TestGraph {
            extends: HashMap::from([("BookType", vec!["DocumentType"])]),
            described_by: HashMap::from([("book-1", vec!["BookType"])]),
            ..Default::default()
        };

        assert_eq!(
            effective_descriptor_lineage(&graph, &"book-1"),
            Ok(vec!["BookType", "DocumentType"])
        );
    }

    #[test]
    fn descriptor_holons_combine_own_and_describing_lineages_without_duplicates() {
        let graph = TestGraph {
            extends: HashMap::from([
                ("BookType", vec!["HolonType"]),
                ("TypeDescriptor", vec!["HolonType"]),
            ]),
            described_by: HashMap::from([("BookType", vec!["TypeDescriptor"])]),
            type_descriptors: HashSet::from(["TypeDescriptor"]),
            ..Default::default()
        };

        assert_eq!(
            effective_descriptor_lineage(&graph, &"BookType"),
            Ok(vec!["BookType", "HolonType", "TypeDescriptor"])
        );
    }

    #[test]
    fn flattening_preserves_lineage_order_and_deduplicates_by_identity() {
        let graph = TestGraph {
            extends: HashMap::from([("Child", vec!["Parent"])]),
            members: HashMap::from([
                (("Child", "properties"), vec!["Name", "Title"]),
                (("Parent", "properties"), vec!["Name", "Description"]),
            ]),
            ..Default::default()
        };

        assert_eq!(
            flatten_related_members(&graph, &"Child", &"properties"),
            Ok(vec!["Name", "Title", "Description"])
        );
    }

    #[test]
    fn multiple_described_by_targets_are_rejected_by_the_kernel() {
        let graph = TestGraph {
            described_by: HashMap::from([("instance", vec!["TypeA", "TypeB"])]),
            ..Default::default()
        };

        assert_eq!(
            effective_descriptor_lineage(&graph, &"instance"),
            Err(DescriptorSemanticsError::MultipleDescribedBy { holon: "instance", count: 2 })
        );
    }
}
