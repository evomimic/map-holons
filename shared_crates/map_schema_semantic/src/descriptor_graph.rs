//! Canonical Holon IR adapter for representation-neutral descriptor semantics.
//!
//! The adapter owns no inheritance or conformance policy. It exposes stable holon identity and raw
//! relationship target collections to `descriptor_semantics`, then projects kernel failures into
//! the shared diagnostic vocabulary at this representation boundary.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use descriptor_semantics::{DescriptorGraph, DescriptorSemanticsError};

use crate::{
    CanonicalHolon, CanonicalReference, Diagnostic, DiagnosticKind, DiagnosticLayer, Origin,
    SemanticModel, SymbolIndex,
};

/// Canonical Core Schema identity used by the default descriptor graph bootstrap contract.
pub const TYPE_DESCRIPTOR_BOOTSTRAP_KEY: &str = "TypeDescriptor.HolonType";

/// Minimal explicit bootstrap identities needed before descriptor data can describe itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalGraphBootstrap {
    pub type_descriptor_keys: Vec<String>,
}

impl Default for CanonicalGraphBootstrap {
    fn default() -> Self {
        Self { type_descriptor_keys: vec![TYPE_DESCRIPTOR_BOOTSTRAP_KEY.to_string()] }
    }
}

/// Build-local handle for a holon in a [`CanonicalDescriptorGraph`].
///
/// The handle is deliberately opaque. [`DescriptorGraph::identity`] returns the canonical holon key
/// used for cycle detection and deduplication, so vector position never becomes semantic identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CanonicalNodeId(usize);

/// Failure while constructing the Canonical Holon IR graph adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalGraphBuildError {
    DuplicateIdentity { key: String },
}

impl fmt::Display for CanonicalGraphBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateIdentity { key } => {
                write!(formatter, "canonical holon identity `{key}` occurs more than once")
            }
        }
    }
}

/// Representation access failure encountered while the semantics kernel traverses Canonical IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalGraphError {
    UnknownNode { node: CanonicalNodeId },
    UnresolvedTarget { source: CanonicalNodeId, relationship: String, target: String },
    ResolvedTargetOutsideModel { source: CanonicalNodeId, relationship: String, target: String },
}

impl fmt::Display for CanonicalGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode { node } => write!(formatter, "unknown canonical node {node:?}"),
            Self::UnresolvedTarget { relationship, target, .. } => {
                write!(formatter, "unresolved `{relationship}` target `{target}`")
            }
            Self::ResolvedTargetOutsideModel { relationship, target, .. } => write!(
                formatter,
                "resolved `{relationship}` target `{target}` is outside the Canonical Holon IR model"
            ),
        }
    }
}

/// Read-only Canonical Holon IR graph consumed by `descriptor_semantics`.
pub struct CanonicalDescriptorGraph {
    holons: Vec<CanonicalHolon>,
    nodes_by_key: HashMap<String, CanonicalNodeId>,
    type_descriptor_nodes: Vec<CanonicalNodeId>,
    schema_nodes: HashSet<CanonicalNodeId>,
    symbols: SymbolIndex,
}

impl CanonicalDescriptorGraph {
    /// Builds an adapter over an indexed Canonical Holon IR model.
    ///
    /// The default bootstrap names only the canonical descriptor-of-descriptors identity needed to
    /// enter the otherwise self-describing graph. No inheritance or effective-member behavior is
    /// derived from projected descriptor kinds; those rules remain in the shared kernel.
    pub fn new(
        model: &SemanticModel,
        symbols: &SymbolIndex,
    ) -> Result<Self, CanonicalGraphBuildError> {
        Self::with_bootstrap(model, symbols, &CanonicalGraphBootstrap::default())
    }

    /// Builds an adapter with an explicit bootstrap identity contract.
    pub fn with_bootstrap(
        model: &SemanticModel,
        symbols: &SymbolIndex,
        bootstrap: &CanonicalGraphBootstrap,
    ) -> Result<Self, CanonicalGraphBuildError> {
        let holons = model.canonical_holons();
        let mut nodes_by_key = HashMap::new();
        for (index, holon) in holons.iter().enumerate() {
            if nodes_by_key.insert(holon.key.clone(), CanonicalNodeId(index)).is_some() {
                return Err(CanonicalGraphBuildError::DuplicateIdentity { key: holon.key.clone() });
            }
        }

        let type_descriptor_nodes = bootstrap
            .type_descriptor_keys
            .iter()
            .filter_map(|key| nodes_by_key.get(key).copied())
            .collect();

        let schema_nodes = model
            .schemas
            .iter()
            .filter_map(|schema| nodes_by_key.get(&schema.key).copied())
            .collect();

        Ok(Self {
            holons,
            nodes_by_key,
            type_descriptor_nodes,
            schema_nodes,
            symbols: symbols.clone(),
        })
    }

    /// Returns the node with the supplied canonical key.
    pub fn node_by_key(&self, key: &str) -> Option<CanonicalNodeId> {
        self.nodes_by_key.get(key).copied()
    }

    /// Iterates all canonical graph nodes in deterministic model order.
    pub fn nodes(&self) -> impl Iterator<Item = CanonicalNodeId> + '_ {
        (0..self.holons.len()).map(CanonicalNodeId)
    }

    /// Returns whether the node is a schema container rather than a descriptor-governed holon.
    pub fn is_schema(&self, node: CanonicalNodeId) -> bool {
        self.schema_nodes.contains(&node)
    }

    /// Returns generic holon data for a graph node.
    pub fn holon(&self, node: CanonicalNodeId) -> Option<&CanonicalHolon> {
        self.holons.get(node.0)
    }

    /// Returns the canonical key for a graph node.
    pub fn key(&self, node: CanonicalNodeId) -> Option<&str> {
        self.holon(node).map(|holon| holon.key.as_str())
    }

    /// Projects a kernel graph failure into one schema-aware diagnostic.
    pub fn diagnostic(
        &self,
        error: DescriptorSemanticsError<CanonicalGraphError, CanonicalNodeId>,
    ) -> Diagnostic {
        match error {
            DescriptorSemanticsError::MultipleExtends { descriptor, count } => {
                self.cardinality_diagnostic(descriptor, "Extends", count)
            }
            DescriptorSemanticsError::MultipleDescribedBy { holon, count } => {
                self.cardinality_diagnostic(holon, "DescribedBy", count)
            }
            DescriptorSemanticsError::MissingDescribedBy { holon } => Diagnostic::error(
                DiagnosticLayer::SchemaAware,
                DiagnosticKind::DescriptorGraphAccess {
                    holon: self.node_label(holon),
                    relationship: "DescribedBy".to_string(),
                    target: None,
                    message: "expected exactly one target; found 0".to_string(),
                },
                self.origin(holon),
            ),
            DescriptorSemanticsError::CyclicExtends { descriptor } => {
                let key = self.node_label(descriptor);
                Diagnostic::error(
                    DiagnosticLayer::SchemaAware,
                    DiagnosticKind::InheritanceCycle { descriptor: key.clone(), target: key },
                    self.origin(descriptor),
                )
            }
            DescriptorSemanticsError::MultipleRelatedMembers { descriptor, kind, count } => {
                self.cardinality_diagnostic(descriptor, kind, count)
            }
            DescriptorSemanticsError::DuplicateInheritedDeclaration { descriptor, kind, name } => {
                Diagnostic::error(
                    DiagnosticLayer::SchemaAware,
                    DiagnosticKind::DescriptorGraphAccess {
                        holon: self.node_label(descriptor),
                        relationship: format!("effective {kind} declarations"),
                        target: Some(name),
                        message: "distinct inherited declarations have the same semantic name"
                            .to_string(),
                    },
                    self.origin(descriptor),
                )
            }
            DescriptorSemanticsError::Access(error) => self.access_diagnostic(error),
        }
    }

    fn relationship_targets(
        &self,
        node: CanonicalNodeId,
        relationship_name: &str,
    ) -> Result<Vec<CanonicalNodeId>, CanonicalGraphError> {
        let holon = self.holon(node).ok_or(CanonicalGraphError::UnknownNode { node })?;
        holon
            .relationships
            .iter()
            .filter(|relationship| relationship.name == relationship_name)
            .flat_map(|relationship| relationship.targets.iter())
            .map(|reference| self.resolve_target(node, relationship_name, reference))
            .collect()
    }

    fn resolve_target(
        &self,
        source: CanonicalNodeId,
        relationship: &str,
        reference: &CanonicalReference,
    ) -> Result<CanonicalNodeId, CanonicalGraphError> {
        let symbol = reference
            .resolved
            .and_then(|symbol_id| self.symbols.lookup_by_id(symbol_id))
            .or_else(|| self.symbols.lookup_reference_target(&reference.target))
            .ok_or_else(|| CanonicalGraphError::UnresolvedTarget {
                source,
                relationship: relationship.to_string(),
                target: reference.target.clone(),
            })?;

        self.nodes_by_key.get(&symbol.key).copied().ok_or_else(|| {
            CanonicalGraphError::ResolvedTargetOutsideModel {
                source,
                relationship: relationship.to_string(),
                target: reference.target.clone(),
            }
        })
    }

    fn origin(&self, node: CanonicalNodeId) -> Option<Origin> {
        self.holon(node).map(|holon| holon.origin.clone())
    }

    fn node_label(&self, node: CanonicalNodeId) -> String {
        self.key(node).map(ToString::to_string).unwrap_or_else(|| format!("{node:?}"))
    }

    fn cardinality_diagnostic(
        &self,
        node: CanonicalNodeId,
        relationship: &str,
        actual: usize,
    ) -> Diagnostic {
        Diagnostic::error(
            DiagnosticLayer::SchemaAware,
            DiagnosticKind::DescriptorRelationshipCardinality {
                holon: self.node_label(node),
                relationship: relationship.to_string(),
                actual,
                maximum: 1,
            },
            self.origin(node),
        )
    }

    fn access_diagnostic(&self, error: CanonicalGraphError) -> Diagnostic {
        let (node, relationship, target) = match &error {
            CanonicalGraphError::UnknownNode { node } => (*node, "graph node".to_string(), None),
            CanonicalGraphError::UnresolvedTarget { source, relationship, target }
            | CanonicalGraphError::ResolvedTargetOutsideModel { source, relationship, target } => {
                (*source, relationship.clone(), Some(target.clone()))
            }
        };
        Diagnostic::error(
            DiagnosticLayer::SchemaAware,
            DiagnosticKind::DescriptorGraphAccess {
                holon: self.node_label(node),
                relationship,
                target,
                message: error.to_string(),
            },
            self.origin(node),
        )
    }
}

impl DescriptorGraph for CanonicalDescriptorGraph {
    type Node = CanonicalNodeId;
    type Identity = String;
    type MemberKind = String;
    type Error = CanonicalGraphError;

    fn identity(&self, node: &Self::Node) -> Self::Identity {
        self.node_label(*node)
    }

    fn extends_targets(&self, node: &Self::Node) -> Result<Vec<Self::Node>, Self::Error> {
        self.relationship_targets(*node, "Extends")
    }

    fn described_by_targets(&self, node: &Self::Node) -> Result<Vec<Self::Node>, Self::Error> {
        let holon = self.holon(*node).ok_or(CanonicalGraphError::UnknownNode { node: *node })?;
        holon
            .described_by
            .iter()
            .map(|reference| self.resolve_target(*node, "DescribedBy", reference))
            .collect()
    }

    fn related_members(
        &self,
        node: &Self::Node,
        member_kind: &Self::MemberKind,
    ) -> Result<Vec<Self::Node>, Self::Error> {
        self.relationship_targets(*node, member_kind)
    }

    fn is_type_descriptor(&self, node: &Self::Node) -> Result<bool, Self::Error> {
        if self.holon(*node).is_none() {
            return Err(CanonicalGraphError::UnknownNode { node: *node });
        }
        Ok(self.type_descriptor_nodes.contains(node))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DescriptorKind, LiteralRelationship, ReferenceRole, SemanticReference, SourceKind,
        TypeDescriptor,
    };

    fn descriptor(key: &str, kind: DescriptorKind) -> TypeDescriptor {
        TypeDescriptor::new(key, key, kind, "TestSchema", Origin::new(SourceKind::Generated))
    }

    fn graph_model() -> (SemanticModel, SymbolIndex) {
        let mut model = SemanticModel::new();
        model.push_descriptor(descriptor(
            "TypeDescriptor.HolonType",
            DescriptorKind::TypeDescriptor,
        ));
        model.push_descriptor(descriptor("Name.PropertyType", DescriptorKind::PropertyType));
        model.push_descriptor(descriptor("Title.PropertyType", DescriptorKind::PropertyType));

        let mut parent = descriptor("Parent.HolonType", DescriptorKind::HolonType);
        parent.instance_properties.push(SemanticReference::unresolved(
            ReferenceRole::InstanceProperty,
            "Name.PropertyType",
        ));
        model.push_descriptor(parent);

        let mut child = descriptor("Child.HolonType", DescriptorKind::HolonType);
        child.described_by.push(SemanticReference::unresolved(
            ReferenceRole::DescribedBy,
            "TypeDescriptor.HolonType",
        ));
        child.extends =
            Some(SemanticReference::unresolved(ReferenceRole::Extends, "Parent.HolonType"));
        child.instance_properties.push(SemanticReference::unresolved(
            ReferenceRole::InstanceProperty,
            "Title.PropertyType",
        ));
        model.push_descriptor(child);

        let (symbols, diagnostics) = SymbolIndex::build(&mut model);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        (model, symbols)
    }

    #[test]
    fn delegates_lineage_and_member_flattening_to_the_shared_kernel() {
        let (model, symbols) = graph_model();
        let graph = CanonicalDescriptorGraph::new(&model, &symbols).expect("graph");
        let child = graph.node_by_key("Child.HolonType").expect("child");

        let lineage = descriptor_semantics::effective_descriptor_lineage(&graph, &child)
            .expect("effective lineage")
            .into_iter()
            .map(|node| graph.key(node).unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            lineage,
            vec!["Child.HolonType", "Parent.HolonType", "TypeDescriptor.HolonType"]
        );

        let properties = descriptor_semantics::flatten_related_members(
            &graph,
            &child,
            &"InstanceProperties".to_string(),
        )
        .expect("flattened properties")
        .into_iter()
        .map(|node| graph.key(node).unwrap().to_string())
        .collect::<Vec<_>>();
        assert_eq!(properties, vec!["Title.PropertyType", "Name.PropertyType"]);
    }

    #[test]
    fn preserves_raw_multiple_extends_targets_for_kernel_cardinality_enforcement() {
        let (mut model, _) = graph_model();
        let child = model
            .descriptors
            .iter_mut()
            .find(|descriptor| descriptor.key == "Child.HolonType")
            .expect("child");
        child.literal_relationships = vec![LiteralRelationship {
            name: "Extends".to_string(),
            targets: vec!["Parent.HolonType".to_string(), "TypeDescriptor.HolonType".to_string()],
        }];
        let (symbols, _) = SymbolIndex::build(&mut model);
        let graph = CanonicalDescriptorGraph::new(&model, &symbols).expect("graph");
        let child = graph.node_by_key("Child.HolonType").expect("child");

        let error = descriptor_semantics::ancestors(&graph, &child).expect_err("multiple Extends");
        assert!(matches!(
            error,
            DescriptorSemanticsError::MultipleExtends { descriptor, count: 2 }
                if descriptor == child
        ));
        assert!(matches!(
            graph.diagnostic(error).kind,
            DiagnosticKind::DescriptorRelationshipCardinality {
                relationship,
                actual: 2,
                maximum: 1,
                ..
            } if relationship == "Extends"
        ));
    }

    #[test]
    fn projects_described_by_cardinality_and_extends_cycles_from_kernel_errors() {
        let (mut model, _) = graph_model();
        let child = model
            .descriptors
            .iter_mut()
            .find(|descriptor| descriptor.key == "Child.HolonType")
            .expect("child");
        child
            .described_by
            .push(SemanticReference::unresolved(ReferenceRole::DescribedBy, "Parent.HolonType"));
        let (symbols, _) = SymbolIndex::build(&mut model);
        let graph = CanonicalDescriptorGraph::new(&model, &symbols).expect("graph");
        let child = graph.node_by_key("Child.HolonType").expect("child");
        let error = descriptor_semantics::effective_descriptor_lineage(&graph, &child)
            .expect_err("multiple DescribedBy");
        assert!(matches!(
            graph.diagnostic(error).kind,
            DiagnosticKind::DescriptorRelationshipCardinality {
                relationship,
                actual: 2,
                maximum: 1,
                ..
            } if relationship == "DescribedBy"
        ));

        let (mut model, _) = graph_model();
        let parent = model
            .descriptors
            .iter_mut()
            .find(|descriptor| descriptor.key == "Parent.HolonType")
            .expect("parent");
        parent.extends =
            Some(SemanticReference::unresolved(ReferenceRole::Extends, "Child.HolonType"));
        let (symbols, _) = SymbolIndex::build(&mut model);
        let graph = CanonicalDescriptorGraph::new(&model, &symbols).expect("graph");
        let child = graph.node_by_key("Child.HolonType").expect("child");
        let error = descriptor_semantics::ancestors(&graph, &child).expect_err("Extends cycle");
        assert!(matches!(
            graph.diagnostic(error).kind,
            DiagnosticKind::InheritanceCycle { descriptor, target }
                if descriptor == "Child.HolonType" && target == "Child.HolonType"
        ));
    }

    #[test]
    fn projects_unresolved_raw_targets_as_graph_access_diagnostics() {
        let (mut model, _) = graph_model();
        let child = model
            .descriptors
            .iter_mut()
            .find(|descriptor| descriptor.key == "Child.HolonType")
            .expect("child");
        child.literal_relationships = vec![LiteralRelationship {
            name: "Extends".to_string(),
            targets: vec!["Missing.HolonType".to_string()],
        }];
        child.extends = None;
        let (symbols, _) = SymbolIndex::build(&mut model);
        let graph = CanonicalDescriptorGraph::new(&model, &symbols).expect("graph");
        let child = graph.node_by_key("Child.HolonType").expect("child");
        let error = descriptor_semantics::ancestors(&graph, &child).expect_err("unresolved target");

        assert!(matches!(
            graph.diagnostic(error).kind,
            DiagnosticKind::DescriptorGraphAccess {
                relationship,
                target: Some(target),
                ..
            } if relationship == "Extends" && target == "Missing.HolonType"
        ));
    }
}
