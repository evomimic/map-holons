//! Shared in-memory semantic model for MAP Schema descriptors.
//!
//! AP02 keeps this model between syntax-specific loaders and emitters:
//! JSON import data and future TDL ASTs should both normalize into these structures before symbol
//! resolution, comparison, or output generation.

use crate::symbols::{SymbolId, SymbolTable};
use std::{collections::BTreeMap, path::PathBuf};

/// Semantic descriptor categories understood by the Milestone A schema tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DescriptorKind {
    Schema,
    TypeDescriptor,
    HolonType,
    PropertyType,
    RelationshipType,
    ValueType,
    Enum,
    EnumVariant,
}

/// Distinguishes declared relationship descriptors from inverse relationship descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelationshipFlavor {
    Declared,
    Inverse,
}

/// Source family for origin metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    JsonImport,
    TdlSource,
    Generated,
}

/// Optional source location attached to schemas, descriptors, symbols, and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub source_kind: SourceKind,
    pub file_path: Option<PathBuf>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

impl Origin {
    pub fn new(source_kind: SourceKind) -> Self {
        Self { source_kind, file_path: None, line: None, column: None }
    }

    pub fn json_file(path: impl Into<PathBuf>) -> Self {
        Self {
            source_kind: SourceKind::JsonImport,
            file_path: Some(path.into()),
            line: None,
            column: None,
        }
    }

    pub fn tdl_file(path: impl Into<PathBuf>, line: Option<u32>, column: Option<u32>) -> Self {
        Self { source_kind: SourceKind::TdlSource, file_path: Some(path.into()), line, column }
    }
}

/// Role a semantic reference plays on its owning descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReferenceRole {
    ComponentOf,
    Extends,
    KeyRule,
    SourceType,
    TargetType,
    InverseOf,
    HasInverse,
    ValueType,
    VariantOf,
    InstanceProperty,
    InstanceRelationship,
    DependsOn,
}

/// Reference text as authored/imported, optionally resolved to a symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticReference {
    pub role: ReferenceRole,
    pub target: String,
    pub resolved: Option<SymbolId>,
}

impl SemanticReference {
    pub fn unresolved(role: ReferenceRole, target: impl Into<String>) -> Self {
        Self { role, target: target.into(), resolved: None }
    }

    pub fn resolve(&mut self, symbol_id: SymbolId) {
        self.resolved = Some(symbol_id);
    }

    pub fn comparable_key(&self) -> (&ReferenceRole, &str) {
        (&self.role, &self.target)
    }
}

/// Human-facing descriptor header fields preserved for later emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorHeader {
    pub description: Option<String>,
    pub display_name: Option<String>,
    pub display_name_plural: Option<String>,
    pub type_name_plural: Option<String>,
}

/// Semantic representation of a schema declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub name: String,
    pub key: String,
    pub origin: Origin,
    pub dependencies: Vec<SemanticReference>,
    pub header: Option<DescriptorHeader>,
    pub allows_additional_properties: bool,
    pub allows_additional_relationships: bool,
}

/// Semantic representation of a non-schema descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDescriptor {
    pub key: String,
    pub name: String,
    pub kind: DescriptorKind,
    pub owning_schema: String,
    pub origin: Origin,
    pub header: Option<DescriptorHeader>,
    pub is_abstract: bool,
    pub extends: Option<SemanticReference>,
    pub component_of: Option<SemanticReference>,
    pub key_rule: Option<SemanticReference>,
    pub value_type: Option<SemanticReference>,
    pub source_type: Option<SemanticReference>,
    pub target_type: Option<SemanticReference>,
    pub inverse_of: Option<SemanticReference>,
    pub has_inverse: Option<SemanticReference>,
    pub variant_of: Option<SemanticReference>,
    pub instance_properties: Vec<SemanticReference>,
    pub instance_relationships: Vec<SemanticReference>,
    pub relationship_flavor: Option<RelationshipFlavor>,
    pub is_definitional: bool,
    pub min_cardinality: Option<i64>,
    pub max_cardinality: Option<i64>,
    pub deletion_semantic: Option<String>,
    pub is_ordered: bool,
    pub allows_duplicates: bool,
    pub allows_additional_properties: bool,
    pub allows_additional_relationships: bool,
}

impl TypeDescriptor {
    /// Creates a descriptor with no optional semantic fields populated.
    pub fn new(
        key: impl Into<String>,
        name: impl Into<String>,
        kind: DescriptorKind,
        owning_schema: impl Into<String>,
        origin: Origin,
    ) -> Self {
        Self {
            key: key.into(),
            name: name.into(),
            kind,
            owning_schema: owning_schema.into(),
            origin,
            header: None,
            is_abstract: false,
            extends: None,
            component_of: None,
            key_rule: None,
            value_type: None,
            source_type: None,
            target_type: None,
            inverse_of: None,
            has_inverse: None,
            variant_of: None,
            instance_properties: Vec::new(),
            instance_relationships: Vec::new(),
            relationship_flavor: None,
            is_definitional: false,
            min_cardinality: None,
            max_cardinality: None,
            deletion_semantic: None,
            is_ordered: false,
            allows_duplicates: false,
            allows_additional_properties: false,
            allows_additional_relationships: false,
        }
    }

    /// Returns all references carried by this descriptor.
    pub fn references(&self) -> Vec<&SemanticReference> {
        let mut refs = Vec::new();
        refs.extend(self.extends.iter());
        refs.extend(self.component_of.iter());
        refs.extend(self.key_rule.iter());
        refs.extend(self.value_type.iter());
        refs.extend(self.source_type.iter());
        refs.extend(self.target_type.iter());
        refs.extend(self.inverse_of.iter());
        refs.extend(self.has_inverse.iter());
        refs.extend(self.variant_of.iter());
        refs.extend(self.instance_properties.iter());
        refs.extend(self.instance_relationships.iter());
        refs
    }

    /// Returns all references carried by this descriptor for resolution.
    pub fn references_mut(&mut self) -> Vec<&mut SemanticReference> {
        let mut refs = Vec::new();
        refs.extend(self.extends.iter_mut());
        refs.extend(self.component_of.iter_mut());
        refs.extend(self.key_rule.iter_mut());
        refs.extend(self.value_type.iter_mut());
        refs.extend(self.source_type.iter_mut());
        refs.extend(self.target_type.iter_mut());
        refs.extend(self.inverse_of.iter_mut());
        refs.extend(self.has_inverse.iter_mut());
        refs.extend(self.variant_of.iter_mut());
        refs.extend(self.instance_properties.iter_mut());
        refs.extend(self.instance_relationships.iter_mut());
        refs
    }
}

/// A source-normalized collection of schemas and descriptors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticModel {
    pub schemas: Vec<Schema>,
    pub descriptors: Vec<TypeDescriptor>,
}

impl SemanticModel {
    /// Creates an empty semantic model.
    pub fn new() -> Self {
        Self { schemas: Vec::new(), descriptors: Vec::new() }
    }

    pub fn push_schema(&mut self, schema: Schema) {
        self.schemas.push(schema);
    }

    pub fn push_descriptor(&mut self, descriptor: TypeDescriptor) {
        self.descriptors.push(descriptor);
    }

    /// Resolves known references in-place against an already-derived symbol table.
    pub fn resolve_references(&mut self, symbols: &SymbolTable) {
        for schema in &mut self.schemas {
            for reference in &mut schema.dependencies {
                if let Some(symbol) = symbols.lookup_by_key(&reference.target) {
                    reference.resolve(symbol.id);
                }
            }
        }

        for descriptor in &mut self.descriptors {
            for reference in descriptor.references_mut() {
                if let Some(symbol) = symbols.lookup_by_key(&reference.target) {
                    reference.resolve(symbol.id);
                } else if let Some(symbol) = symbols.lookup_by_name(&reference.target).first() {
                    reference.resolve(symbol.id);
                }
            }
        }
    }

    /// Produces a stable, origin-insensitive signature for round-trip comparisons.
    pub fn comparable_signature(&self) -> ComparableSemanticModel {
        let mut schemas = self
            .schemas
            .iter()
            .map(|schema| ComparableSchema {
                name: schema.name.clone(),
                key: schema.key.clone(),
                dependencies: comparable_refs(&schema.dependencies),
                allows_additional_properties: schema.allows_additional_properties,
                allows_additional_relationships: schema.allows_additional_relationships,
            })
            .collect::<Vec<_>>();
        schemas.sort();

        let mut descriptors = self
            .descriptors
            .iter()
            .map(|descriptor| ComparableDescriptor {
                key: descriptor.key.clone(),
                name: descriptor.name.clone(),
                kind: descriptor.kind,
                owning_schema: descriptor.owning_schema.clone(),
                is_abstract: descriptor.is_abstract,
                references: comparable_descriptor_refs(descriptor),
                relationship_flavor: descriptor.relationship_flavor,
                is_definitional: descriptor.is_definitional,
                min_cardinality: descriptor.min_cardinality,
                max_cardinality: descriptor.max_cardinality,
                deletion_semantic: descriptor.deletion_semantic.clone(),
                is_ordered: descriptor.is_ordered,
                allows_duplicates: descriptor.allows_duplicates,
                allows_additional_properties: descriptor.allows_additional_properties,
                allows_additional_relationships: descriptor.allows_additional_relationships,
            })
            .collect::<Vec<_>>();
        descriptors.sort();

        ComparableSemanticModel { schemas, descriptors }
    }
}

impl Default for SemanticModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Origin-insensitive semantic model representation for equivalence checks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComparableSemanticModel {
    pub schemas: Vec<ComparableSchema>,
    pub descriptors: Vec<ComparableDescriptor>,
}

/// Origin-insensitive schema representation for equivalence checks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComparableSchema {
    pub name: String,
    pub key: String,
    pub dependencies: Vec<(ReferenceRole, String)>,
    pub allows_additional_properties: bool,
    pub allows_additional_relationships: bool,
}

/// Origin-insensitive descriptor representation for equivalence checks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComparableDescriptor {
    pub key: String,
    pub name: String,
    pub kind: DescriptorKind,
    pub owning_schema: String,
    pub is_abstract: bool,
    pub references: Vec<(ReferenceRole, String)>,
    pub relationship_flavor: Option<RelationshipFlavor>,
    pub is_definitional: bool,
    pub min_cardinality: Option<i64>,
    pub max_cardinality: Option<i64>,
    pub deletion_semantic: Option<String>,
    pub is_ordered: bool,
    pub allows_duplicates: bool,
    pub allows_additional_properties: bool,
    pub allows_additional_relationships: bool,
}

fn comparable_descriptor_refs(descriptor: &TypeDescriptor) -> Vec<(ReferenceRole, String)> {
    let mut refs = descriptor
        .references()
        .into_iter()
        .map(|reference| (reference.role, reference.target.clone()))
        .collect::<Vec<_>>();
    refs.sort();
    refs
}

fn comparable_refs(references: &[SemanticReference]) -> Vec<(ReferenceRole, String)> {
    let mut refs = references
        .iter()
        .map(|reference| (reference.role, reference.target.clone()))
        .collect::<Vec<_>>();
    refs.sort();
    refs
}

/// Minimal adapter input for future TDL AST-to-semantic tests and loaders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TdlDescriptorInput {
    pub key: String,
    pub name: String,
    pub kind: DescriptorKind,
    pub owning_schema: String,
    pub origin: Origin,
    pub references: BTreeMap<ReferenceRole, Vec<String>>,
}

impl TdlDescriptorInput {
    pub fn into_descriptor(self) -> TypeDescriptor {
        let mut descriptor =
            TypeDescriptor::new(self.key, self.name, self.kind, self.owning_schema, self.origin);
        for (role, targets) in self.references {
            for target in targets {
                push_reference(&mut descriptor, SemanticReference::unresolved(role, target));
            }
        }
        descriptor
    }
}

/// Adds a reference to the matching semantic slot on a descriptor.
pub fn push_reference(descriptor: &mut TypeDescriptor, reference: SemanticReference) {
    match reference.role {
        ReferenceRole::ComponentOf => descriptor.component_of = Some(reference),
        ReferenceRole::Extends => descriptor.extends = Some(reference),
        ReferenceRole::KeyRule => descriptor.key_rule = Some(reference),
        ReferenceRole::SourceType => descriptor.source_type = Some(reference),
        ReferenceRole::TargetType => descriptor.target_type = Some(reference),
        ReferenceRole::InverseOf => descriptor.inverse_of = Some(reference),
        ReferenceRole::HasInverse => descriptor.has_inverse = Some(reference),
        ReferenceRole::ValueType => descriptor.value_type = Some(reference),
        ReferenceRole::VariantOf => descriptor.variant_of = Some(reference),
        ReferenceRole::InstanceProperty => descriptor.instance_properties.push(reference),
        ReferenceRole::InstanceRelationship => descriptor.instance_relationships.push(reference),
        ReferenceRole::DependsOn => {}
    }
}
