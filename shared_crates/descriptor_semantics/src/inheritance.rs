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

/// Returns the exactly one concrete type that describes `holon`.
pub fn describing_type<G: DescriptorGraph>(
    graph: &G,
    holon: &G::Node,
) -> Result<G::Node, DescriptorSemanticsError<G::Error, G::Node>> {
    let targets = graph.described_by_targets(holon).map_err(DescriptorSemanticsError::Access)?;
    match targets.as_slice() {
        [] => Err(DescriptorSemanticsError::MissingDescribedBy { holon: holon.clone() }),
        [descriptor] => Ok(descriptor.clone()),
        many => Err(DescriptorSemanticsError::MultipleDescribedBy {
            holon: holon.clone(),
            count: many.len(),
        }),
    }
}

/// Resolves the effective key rule governing instances described by `type_descriptor`.
///
/// `UsesKeyRule` is inherited self-first through the descriptor's `Extends` lineage. A lineage
/// node may declare at most one rule.
pub fn effective_instance_key_rule<G: DescriptorGraph>(
    graph: &G,
    type_descriptor: &G::Node,
    uses_key_rule: &G::MemberKind,
) -> Result<Option<G::Node>, DescriptorSemanticsError<G::Error, G::Node>> {
    first_single_member_from_lineage(
        graph,
        ancestors(graph, type_descriptor)?,
        uses_key_rule,
        "UsesKeyRule",
    )
}

/// Resolves the key rule governing `holon` itself.
///
/// A descriptor holon's own direct `UsesKeyRule` governs its instances, so own-key resolution
/// starts at its direct `Extends` parent. If that lineage supplies no rule, resolution falls back
/// to the actual `DescribedBy` type and its effective instance rule.
pub fn effective_holon_key_rule<G: DescriptorGraph>(
    graph: &G,
    holon: &G::Node,
    uses_key_rule: &G::MemberKind,
) -> Result<Option<G::Node>, DescriptorSemanticsError<G::Error, G::Node>> {
    let described_by = describing_type(graph, holon)?;
    let own_lineage = lineage_with_describing_type(graph, holon, &described_by)?;
    if let Some(rule) = first_single_member_from_lineage(
        graph,
        own_lineage.into_iter().skip(1),
        uses_key_rule,
        "UsesKeyRule",
    )? {
        return Ok(Some(rule));
    }

    effective_instance_key_rule(graph, &described_by, uses_key_rule)
}

fn first_single_member_from_lineage<G: DescriptorGraph>(
    graph: &G,
    lineage: impl IntoIterator<Item = G::Node>,
    member_kind: &G::MemberKind,
    kind_label: &'static str,
) -> Result<Option<G::Node>, DescriptorSemanticsError<G::Error, G::Node>> {
    for descriptor in lineage {
        let members = graph
            .related_members(&descriptor, member_kind)
            .map_err(DescriptorSemanticsError::Access)?;
        match members.as_slice() {
            [] => {}
            [member] => return Ok(Some(member.clone())),
            many => {
                return Err(DescriptorSemanticsError::MultipleRelatedMembers {
                    descriptor,
                    kind: kind_label,
                    count: many.len(),
                })
            }
        }
    }
    Ok(None)
}

/// Computes `L(H)`, the holon's own type lineage.
///
/// A holon is a type exactly when its unique `DescribedBy` target is `TypeDescriptor`. Type
/// lineages are self-first; ordinary holons have an empty own lineage.
pub fn lineage<G: DescriptorGraph>(
    graph: &G,
    holon: &G::Node,
) -> Result<Vec<G::Node>, DescriptorSemanticsError<G::Error, G::Node>> {
    let described_by = describing_type(graph, holon)?;
    lineage_with_describing_type(graph, holon, &described_by)
}

fn lineage_with_describing_type<G: DescriptorGraph>(
    graph: &G,
    holon: &G::Node,
    described_by: &G::Node,
) -> Result<Vec<G::Node>, DescriptorSemanticsError<G::Error, G::Node>> {
    if graph.is_type_descriptor(described_by).map_err(DescriptorSemanticsError::Access)? {
        ancestors(graph, holon)
    } else {
        Ok(Vec::new())
    }
}

/// Computes `E(H) = L(H) union-by-identity L(D(H))`.
///
/// The own lineage is retained first. Nodes from the describing type's lineage are appended only
/// when their graph identity has not already appeared.
pub fn effective_descriptor_lineage<G: DescriptorGraph>(
    graph: &G,
    holon: &G::Node,
) -> Result<Vec<G::Node>, DescriptorSemanticsError<G::Error, G::Node>> {
    let described_by = describing_type(graph, holon)?;
    let mut effective = lineage_with_describing_type(graph, holon, &described_by)?;
    append_unique(graph, &mut effective, ancestors(graph, &described_by)?);
    Ok(effective)
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

/// Flattens inherited members and rejects distinct declarations with the same semantic name.
///
/// Repeated references are first deduplicated by node identity. A repeated name therefore only
/// fails when it belongs to two distinct declaration nodes in the effective type lineage.
pub fn flatten_named_members<G, F>(
    graph: &G,
    start: &G::Node,
    member_kind: &G::MemberKind,
    declaration_kind: &'static str,
    semantic_name: F,
) -> Result<Vec<G::Node>, DescriptorSemanticsError<G::Error, G::Node>>
where
    G: DescriptorGraph,
    F: FnMut(&G::Node) -> Result<String, G::Error>,
{
    let lineage = ancestors(graph, start)?;
    collect_named_members_from_lineage(graph, lineage, member_kind, declaration_kind, semantic_name)
}

/// Collects named declarations from a caller-selected self-first lineage.
pub fn collect_named_members_from_lineage<G, F>(
    graph: &G,
    lineage: impl IntoIterator<Item = G::Node>,
    member_kind: &G::MemberKind,
    declaration_kind: &'static str,
    mut semantic_name: F,
) -> Result<Vec<G::Node>, DescriptorSemanticsError<G::Error, G::Node>>
where
    G: DescriptorGraph,
    F: FnMut(&G::Node) -> Result<String, G::Error>,
{
    let mut members = Vec::new();
    let mut seen_members = HashSet::new();
    let mut names = Vec::new();
    let mut subject = None;
    for anchor in lineage {
        if subject.is_none() {
            subject = Some(anchor.clone());
        }
        for member in
            graph.related_members(&anchor, member_kind).map_err(DescriptorSemanticsError::Access)?
        {
            if !seen_members.insert(graph.identity(&member)) {
                continue;
            }
            let name = semantic_name(&member).map_err(DescriptorSemanticsError::Access)?;
            names.push(name);
            members.push(member);
        }
    }
    if let Some(name) = duplicate_declaration_name(names) {
        return Err(DescriptorSemanticsError::DuplicateInheritedDeclaration {
            descriptor: subject.expect("a collected member always has a lineage anchor"),
            kind: declaration_kind,
            name,
        });
    }
    Ok(members)
}

/// Returns the first repeated semantic declaration name, if any.
///
/// Callers must deduplicate repeated references by declaration identity before supplying names;
/// only distinct declarations sharing a name are invalid.
pub fn duplicate_declaration_name(names: impl IntoIterator<Item = String>) -> Option<String> {
    let mut seen = HashSet::new();
    names.into_iter().find(|name| !seen.insert(name.clone()))
}

fn append_unique<G: DescriptorGraph>(
    graph: &G,
    lineage: &mut Vec<G::Node>,
    additional: Vec<G::Node>,
) {
    let mut seen = lineage.iter().map(|node| graph.identity(node)).collect::<HashSet<_>>();
    for node in additional {
        if seen.insert(graph.identity(&node)) {
            lineage.push(node);
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
            described_by: HashMap::from([
                ("book-1", vec!["BookType"]),
                ("BookType", vec!["TypeDescriptor"]),
            ]),
            type_descriptors: HashSet::from(["TypeDescriptor"]),
            ..Default::default()
        };

        assert_eq!(
            effective_descriptor_lineage(&graph, &"book-1"),
            Ok(vec!["BookType", "DocumentType"])
        );
    }

    #[test]
    fn type_holons_combine_own_and_describing_lineages_without_duplicates() {
        let graph = TestGraph {
            extends: HashMap::from([
                ("BookType", vec!["HolonType"]),
                ("TypeDescriptor", vec!["HolonType"]),
            ]),
            described_by: HashMap::from([
                ("BookType", vec!["TypeDescriptor"]),
                ("TypeDescriptor", vec!["TypeDescriptor"]),
            ]),
            type_descriptors: HashSet::from(["TypeDescriptor"]),
            ..Default::default()
        };

        assert_eq!(
            effective_descriptor_lineage(&graph, &"BookType"),
            Ok(vec!["BookType", "HolonType", "TypeDescriptor"])
        );
    }

    #[test]
    fn self_describing_type_has_one_identity_deduplicated_lineage() {
        let graph = TestGraph {
            extends: HashMap::from([("TypeDescriptor", vec!["HolonType"])]),
            described_by: HashMap::from([("TypeDescriptor", vec!["TypeDescriptor"])]),
            type_descriptors: HashSet::from(["TypeDescriptor"]),
            ..Default::default()
        };

        assert_eq!(
            effective_descriptor_lineage(&graph, &"TypeDescriptor"),
            Ok(vec!["TypeDescriptor", "HolonType"])
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

    #[test]
    fn missing_described_by_is_rejected_by_the_validity_gate() {
        let graph = TestGraph::default();
        assert_eq!(
            describing_type(&graph, &"instance"),
            Err(DescriptorSemanticsError::MissingDescribedBy { holon: "instance" })
        );
    }

    #[test]
    fn missing_described_by_is_not_reinterpreted_as_own_lineage() {
        let graph =
            TestGraph { extends: HashMap::from([("Child", vec!["Parent"])]), ..Default::default() };

        assert_eq!(
            effective_descriptor_lineage(&graph, &"Child"),
            Err(DescriptorSemanticsError::MissingDescribedBy { holon: "Child" })
        );
    }

    #[test]
    fn named_member_flattening_rejects_distinct_duplicate_declarations() {
        let graph = TestGraph {
            extends: HashMap::from([("Child", vec!["Parent"])]),
            members: HashMap::from([
                (("Child", "properties"), vec!["ChildName"]),
                (("Parent", "properties"), vec!["ParentName"]),
            ]),
            ..Default::default()
        };

        assert_eq!(
            flatten_named_members(&graph, &"Child", &"properties", "property", |_| {
                Ok("name".to_string())
            }),
            Err(DescriptorSemanticsError::DuplicateInheritedDeclaration {
                descriptor: "Child",
                kind: "property",
                name: "name".to_string(),
            })
        );
    }

    #[test]
    fn duplicate_name_policy_reports_the_first_repeated_name() {
        assert_eq!(
            duplicate_declaration_name([
                "title".to_string(),
                "author".to_string(),
                "title".to_string(),
            ]),
            Some("title".to_string())
        );
    }

    #[test]
    fn instance_key_rule_is_self_first_but_descriptor_own_key_excludes_self() {
        let graph = TestGraph {
            extends: HashMap::from([("BookType", vec!["HolonType"])]),
            described_by: HashMap::from([("BookType", vec!["TypeDescriptor"])]),
            members: HashMap::from([
                (("BookType", "key-rule"), vec!["BookRule"]),
                (("HolonType", "key-rule"), vec!["ExtendedTypeRule"]),
                (("TypeDescriptor", "key-rule"), vec!["TypeNameRule"]),
            ]),
            type_descriptors: HashSet::from(["TypeDescriptor"]),
            ..Default::default()
        };

        assert_eq!(
            effective_instance_key_rule(&graph, &"BookType", &"key-rule"),
            Ok(Some("BookRule"))
        );
        assert_eq!(
            effective_holon_key_rule(&graph, &"BookType", &"key-rule"),
            Ok(Some("ExtendedTypeRule"))
        );
    }

    #[test]
    fn holon_key_rule_falls_back_through_described_by() {
        let graph = TestGraph {
            described_by: HashMap::from([
                ("book-1", vec!["BookType"]),
                ("BookType", vec!["TypeDescriptor"]),
            ]),
            members: HashMap::from([(("BookType", "key-rule"), vec!["BookRule"])]),
            type_descriptors: HashSet::from(["TypeDescriptor"]),
            ..Default::default()
        };

        assert_eq!(effective_holon_key_rule(&graph, &"book-1", &"key-rule"), Ok(Some("BookRule")));
    }

    #[test]
    fn root_type_key_excludes_its_own_instance_rule_and_falls_back_to_described_by() {
        let graph = TestGraph {
            described_by: HashMap::from([("RootType", vec!["TypeDescriptor"])]),
            members: HashMap::from([
                (("RootType", "key-rule"), vec!["RootInstanceRule"]),
                (("TypeDescriptor", "key-rule"), vec!["TypeNameRule"]),
            ]),
            type_descriptors: HashSet::from(["TypeDescriptor"]),
            ..Default::default()
        };

        assert_eq!(
            effective_holon_key_rule(&graph, &"RootType", &"key-rule"),
            Ok(Some("TypeNameRule"))
        );
    }

    #[test]
    fn ordinary_holon_ignores_its_own_extends_edge_during_key_resolution() {
        let graph = TestGraph {
            extends: HashMap::from([("book-1", vec!["NotItsType"])]),
            described_by: HashMap::from([
                ("book-1", vec!["BookType"]),
                ("BookType", vec!["TypeDescriptor"]),
            ]),
            members: HashMap::from([
                (("NotItsType", "key-rule"), vec!["WrongRule"]),
                (("BookType", "key-rule"), vec!["BookRule"]),
            ]),
            type_descriptors: HashSet::from(["TypeDescriptor"]),
            ..Default::default()
        };

        assert_eq!(effective_holon_key_rule(&graph, &"book-1", &"key-rule"), Ok(Some("BookRule")));
    }

    #[test]
    fn holon_key_resolution_validates_described_by_before_using_own_ancestors() {
        let graph = TestGraph {
            extends: HashMap::from([("BookType", vec!["HolonType"])]),
            members: HashMap::from([(("HolonType", "key-rule"), vec!["InheritedRule"])]),
            ..Default::default()
        };

        assert_eq!(
            effective_holon_key_rule(&graph, &"BookType", &"key-rule"),
            Err(DescriptorSemanticsError::MissingDescribedBy { holon: "BookType" })
        );
    }
}
