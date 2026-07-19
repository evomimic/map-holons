//! Derived lookup indexes for the Canonical Holon IR.
//!
//! `SymbolIndex` is intentionally build-local and in-memory only. It is a
//! service over [`crate::schema_ir::SemanticModel`], not a second semantic
//! source of truth.

use crate::{
    diagnostics::{Diagnostic, DiagnosticKind, DiagnosticLayer},
    schema_ir::{
        DescriptorKind, Origin, ReferenceRole, Schema, SemanticModel, SemanticReference,
        TypeDescriptor,
    },
};
use std::collections::{BTreeSet, HashMap};

/// Stable symbol identity within one derived symbol index.
///
/// IDs are deterministic for a single index build but are not portable across model mutations,
/// process runs, or serialized artifacts. Persist reference text instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub usize);

/// Symbol metadata derived from a schema or descriptor.
///
/// Symbols are lookup records, not descriptors. The semantic model remains the source of truth for
/// descriptor content and reference roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub key: String,
    pub name: String,
    pub kind: DescriptorKind,
    pub owning_schema: Option<String>,
    pub origin: Origin,
}

/// In-memory indexes derived from a semantic model.
///
/// `SymbolIndex` resolves authored reference text and reports structural issues such as duplicate
/// keys, unresolved references, and role/kind mismatches. It deliberately stores only derived lookup
/// state so callers can rebuild it whenever the semantic model changes.
#[derive(Debug, Clone, Default)]
pub struct SymbolIndex {
    symbols: Vec<Symbol>,
    by_key: HashMap<String, SymbolId>,
    by_name: HashMap<String, Vec<SymbolId>>,
    by_kind: HashMap<DescriptorKind, Vec<SymbolId>>,
    by_schema: HashMap<String, Vec<SymbolId>>,
    duplicate_keys: BTreeSet<String>,
}

impl SymbolIndex {
    /// Creates an empty symbol index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a derived index, resolves references in the model, and returns diagnostics.
    ///
    /// Resolution mutates only the `resolved` fields on semantic references. All authored targets
    /// remain intact for diagnostics, projections, and origin-insensitive comparisons.
    pub fn build(model: &mut SemanticModel) -> (Self, Vec<Diagnostic>) {
        let mut index = Self::new();
        let mut diagnostics = Vec::new();

        for schema in &model.schemas {
            if let Err(diagnostic) = index.insert_schema(schema) {
                diagnostics.push(diagnostic);
            }
        }

        for descriptor in &model.descriptors {
            if let Err(diagnostic) = index.insert_descriptor(descriptor) {
                diagnostics.push(diagnostic);
            }
        }

        model.resolve_references(&index);
        diagnostics.extend(index.collect_reference_diagnostics(model));
        (index, diagnostics)
    }

    /// Backward-compatible constructor name while callers migrate from `SymbolTable`.
    pub fn from_model(model: &mut SemanticModel) -> (Self, Vec<Diagnostic>) {
        Self::build(model)
    }

    /// Inserts one schema symbol.
    pub fn insert_schema(&mut self, schema: &Schema) -> Result<SymbolId, Diagnostic> {
        self.insert(Symbol {
            id: SymbolId(self.symbols.len()),
            key: schema.key.clone(),
            name: schema.name.clone(),
            kind: DescriptorKind::Schema,
            owning_schema: None,
            origin: schema.origin.clone(),
        })
    }

    /// Inserts one descriptor symbol.
    pub fn insert_descriptor(
        &mut self,
        descriptor: &TypeDescriptor,
    ) -> Result<SymbolId, Diagnostic> {
        self.insert(Symbol {
            id: SymbolId(self.symbols.len()),
            key: descriptor.key.clone(),
            name: descriptor.name.clone(),
            kind: descriptor.kind,
            owning_schema: Some(descriptor.owning_schema.clone()),
            origin: descriptor.origin.clone(),
        })
    }

    /// Inserts one prepared symbol into all lookup indexes.
    ///
    /// The supplied `id` is ignored and replaced with the next local [`SymbolId`] so all indexes
    /// stay internally consistent.
    pub fn insert(&mut self, mut symbol: Symbol) -> Result<SymbolId, Diagnostic> {
        if self.by_key.contains_key(&symbol.key) {
            self.duplicate_keys.insert(symbol.key.clone());
            return Err(Diagnostic::error(
                DiagnosticLayer::ReferenceSymbol,
                DiagnosticKind::DuplicateSymbol { key: symbol.key.clone() },
                Some(symbol.origin),
            ));
        }

        let id = SymbolId(self.symbols.len());
        symbol.id = id;
        self.by_key.insert(symbol.key.clone(), id);
        self.by_name.entry(symbol.name.clone()).or_default().push(id);
        self.by_kind.entry(symbol.kind).or_default().push(id);
        if let Some(schema) = &symbol.owning_schema {
            self.by_schema.entry(schema.clone()).or_default().push(id);
        }
        self.symbols.push(symbol);
        Ok(id)
    }

    /// Looks up a symbol by stable ID.
    pub fn lookup_by_id(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.0)
    }

    /// Looks up a symbol by canonical key.
    pub fn lookup_by_key(&self, key: &str) -> Option<&Symbol> {
        self.lookup_by_key_internal(key)
    }

    /// Looks up symbols by display/source name.
    pub fn lookup_by_name(&self, name: &str) -> Vec<&Symbol> {
        self.by_name
            .get(name)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| self.lookup_by_id(*id))
            .collect()
    }

    /// Looks up symbols by descriptor kind.
    pub fn lookup_by_kind(&self, kind: DescriptorKind) -> Vec<&Symbol> {
        self.by_kind
            .get(&kind)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| self.lookup_by_id(*id))
            .collect()
    }

    /// Looks up descriptor symbols owned by a schema.
    pub fn lookup_by_schema(&self, schema: &str) -> Vec<&Symbol> {
        self.by_schema
            .get(schema)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| self.lookup_by_id(*id))
            .collect()
    }

    /// Looks up a symbol by an authored reference target.
    ///
    /// Exact keys and names are tried first. As a migration affordance, suffix-qualified aliases
    /// such as `Schema.HolonType` can resolve to the underlying `Schema` symbol when appropriate.
    pub fn lookup_reference_target(&self, target: &str) -> Option<&Symbol> {
        self.lookup_by_key_internal(target)
    }

    /// Returns duplicate keys encountered during insertion.
    pub fn duplicate_keys(&self) -> impl Iterator<Item = &String> {
        self.duplicate_keys.iter()
    }

    /// Returns all non-duplicate symbols in insertion order.
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    /// Collects references that remain unresolved after symbol resolution.
    pub fn collect_unresolved_references(&self, model: &SemanticModel) -> Vec<SemanticReference> {
        let mut unresolved = Vec::new();
        for schema in &model.schemas {
            unresolved.extend(
                schema
                    .described_by
                    .iter()
                    .chain(schema.dependencies.iter())
                    .filter(|reference| reference.resolved.is_none())
                    .cloned(),
            );
        }
        for descriptor in &model.descriptors {
            unresolved.extend(
                descriptor
                    .references()
                    .into_iter()
                    .filter(|reference| reference.resolved.is_none())
                    .cloned(),
            );
        }
        unresolved
    }

    /// Validates reference resolution and role/kind compatibility after resolution.
    ///
    /// This check is intentionally semantic-role based: a `ValueType` reference must target a value
    /// type or enum regardless of whether it came from JSON import data, TDL syntax, or generated
    /// schema content.
    pub fn collect_reference_diagnostics(&self, model: &SemanticModel) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for schema in &model.schemas {
            for reference in schema.described_by.iter().chain(schema.dependencies.iter()) {
                push_reference_diagnostic(
                    &mut diagnostics,
                    self,
                    reference,
                    Some(schema.origin.clone()),
                );
            }
        }

        for descriptor in &model.descriptors {
            for reference in descriptor.references() {
                push_reference_diagnostic(
                    &mut diagnostics,
                    self,
                    reference,
                    Some(descriptor.origin.clone()),
                );
            }
        }

        diagnostics
    }
}

impl SymbolIndex {
    fn lookup_by_key_internal(&self, key: &str) -> Option<&Symbol> {
        self.by_key
            .get(key)
            .and_then(|id| self.lookup_by_id(*id))
            .or_else(|| self.lookup_by_name(key).into_iter().next())
            .or_else(|| {
                key.rsplit_once('.').and_then(|(base, _suffix)| {
                    self.by_key
                        .get(base)
                        .and_then(|id| self.lookup_by_id(*id))
                        .or_else(|| self.lookup_by_name(base).into_iter().next())
                })
            })
    }
}

fn push_reference_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    index: &SymbolIndex,
    reference: &SemanticReference,
    origin: Option<Origin>,
) {
    let Some(symbol_id) = reference.resolved else {
        diagnostics.push(Diagnostic::error(
            DiagnosticLayer::ReferenceSymbol,
            DiagnosticKind::UnresolvedReference {
                role: reference.role,
                target: reference.target.clone(),
            },
            origin,
        ));
        return;
    };

    let Some(symbol) = index.lookup_by_id(symbol_id) else {
        return;
    };

    let expected = expected_kinds(reference.role);
    if !expected.is_empty() && !expected.contains(&symbol.kind) {
        diagnostics.push(Diagnostic::error(
            DiagnosticLayer::ReferenceSymbol,
            DiagnosticKind::WrongDescriptorKind {
                role: reference.role,
                target: reference.target.clone(),
                actual: symbol.kind,
                expected,
            },
            origin,
        ));
    }
}

fn expected_kinds(role: ReferenceRole) -> Vec<DescriptorKind> {
    match role {
        ReferenceRole::DescribedBy => Vec::new(),
        ReferenceRole::ComponentOf | ReferenceRole::DependsOn => vec![DescriptorKind::Schema],
        ReferenceRole::SourceType | ReferenceRole::TargetType => vec![
            DescriptorKind::TypeDescriptor,
            DescriptorKind::HolonType,
            DescriptorKind::PropertyType,
            DescriptorKind::RelationshipType,
            DescriptorKind::ValueType,
            DescriptorKind::Enum,
            DescriptorKind::EnumVariant,
        ],
        ReferenceRole::ValueType => vec![DescriptorKind::ValueType, DescriptorKind::Enum],
        ReferenceRole::Variants => vec![DescriptorKind::EnumVariant],
        ReferenceRole::VariantOf => vec![DescriptorKind::Enum],
        ReferenceRole::InstanceProperty => vec![DescriptorKind::PropertyType],
        ReferenceRole::InstanceRelationship => vec![DescriptorKind::RelationshipType],
        ReferenceRole::InverseOf | ReferenceRole::HasInverse => {
            vec![DescriptorKind::RelationshipType]
        }
        ReferenceRole::Extends | ReferenceRole::KeyRule => Vec::new(),
    }
}

/// Backward-compatible name for callers still referring to `SymbolTable`.
pub type SymbolTable = SymbolIndex;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal_value::LiteralObject;
    use crate::{
        diagnostics::DiagnosticSeverity,
        schema_ir::{push_reference, TdlDescriptorInput},
    };
    use std::{collections::BTreeMap, path::PathBuf};

    fn json_origin() -> Origin {
        Origin::json_file("schema.json")
    }

    fn tdl_origin() -> Origin {
        Origin::tdl_file("schema.tdl", Some(3), Some(7))
    }

    fn schema(origin: Origin) -> Schema {
        Schema {
            name: "Test Schema".to_string(),
            key: "Test Schema".to_string(),
            origin,
            described_by: Vec::new(),
            dependencies: Vec::new(),
            literal_properties: LiteralObject::new(),
            literal_relationships: Vec::new(),
            header: None,
            allows_additional_properties: false,
            allows_additional_relationships: false,
        }
    }

    fn descriptor(key: &str, name: &str, kind: DescriptorKind, origin: Origin) -> TypeDescriptor {
        TypeDescriptor::new(key, name, kind, "Test Schema", origin)
    }

    #[test]
    fn creates_symbols_from_json_derived_descriptors() {
        let mut model = SemanticModel::new();
        model.push_schema(schema(json_origin()));
        model.push_descriptor(descriptor(
            "MapStringValueType",
            "MapStringValueType",
            DescriptorKind::ValueType,
            json_origin(),
        ));
        model.push_descriptor(descriptor(
            "Name.PropertyType",
            "Name",
            DescriptorKind::PropertyType,
            json_origin(),
        ));

        let (symbols, diagnostics) = SymbolIndex::build(&mut model);

        assert!(diagnostics.is_empty());
        assert_eq!(symbols.symbols().len(), 3);
        assert_eq!(
            symbols.lookup_by_key("Name.PropertyType").map(|symbol| symbol.kind),
            Some(DescriptorKind::PropertyType)
        );
        assert_eq!(symbols.lookup_by_name("Name").len(), 1);
        assert_eq!(symbols.lookup_by_kind(DescriptorKind::ValueType).len(), 1);
        assert_eq!(symbols.lookup_by_schema("Test Schema").len(), 2);
    }

    #[test]
    fn creates_symbols_from_tdl_derived_descriptors() {
        let mut references = BTreeMap::new();
        references.insert(ReferenceRole::ValueType, vec!["MapStringValueType".to_string()]);
        let property = TdlDescriptorInput {
            key: "Name.PropertyType".to_string(),
            name: "Name".to_string(),
            kind: DescriptorKind::PropertyType,
            owning_schema: "Test Schema".to_string(),
            origin: tdl_origin(),
            references,
        }
        .into_descriptor();

        let mut model = SemanticModel::new();
        model.push_schema(schema(tdl_origin()));
        model.push_descriptor(descriptor(
            "MapStringValueType",
            "MapStringValueType",
            DescriptorKind::ValueType,
            tdl_origin(),
        ));
        model.push_descriptor(property);

        let (symbols, diagnostics) = SymbolIndex::build(&mut model);

        assert!(diagnostics.is_empty());
        let property = model
            .descriptors
            .iter()
            .find(|descriptor| descriptor.key == "Name.PropertyType")
            .expect("property descriptor");
        let resolved = property.value_type.as_ref().and_then(|reference| reference.resolved);
        assert_eq!(
            resolved.and_then(|id| symbols.lookup_by_id(id)).map(|symbol| symbol.key.as_str()),
            Some("MapStringValueType")
        );
    }

    #[test]
    fn resolves_references_across_descriptors() {
        let mut property =
            descriptor("Name.PropertyType", "Name", DescriptorKind::PropertyType, json_origin());
        push_reference(
            &mut property,
            SemanticReference::unresolved(ReferenceRole::ValueType, "MapStringValueType"),
        );

        let mut model = SemanticModel::new();
        model.push_schema(schema(json_origin()));
        model.push_descriptor(descriptor(
            "MapStringValueType",
            "MapStringValueType",
            DescriptorKind::ValueType,
            json_origin(),
        ));
        model.push_descriptor(property);

        let (symbols, diagnostics) = SymbolIndex::build(&mut model);

        assert!(diagnostics.is_empty());
        let resolved =
            model.descriptors[1].value_type.as_ref().and_then(|reference| reference.resolved);
        assert_eq!(
            resolved.and_then(|id| symbols.lookup_by_id(id)).map(|symbol| symbol.name.as_str()),
            Some("MapStringValueType")
        );
    }

    #[test]
    fn detects_duplicate_symbols() {
        let mut model = SemanticModel::new();
        model.push_schema(schema(json_origin()));
        model.push_descriptor(descriptor(
            "Name.PropertyType",
            "Name",
            DescriptorKind::PropertyType,
            json_origin(),
        ));
        model.push_descriptor(descriptor(
            "Name.PropertyType",
            "Name",
            DescriptorKind::PropertyType,
            tdl_origin(),
        ));

        let (symbols, diagnostics) = SymbolIndex::build(&mut model);

        assert_eq!(symbols.symbols().len(), 2);
        assert_eq!(
            symbols.duplicate_keys().cloned().collect::<Vec<_>>(),
            vec!["Name.PropertyType"]
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == DiagnosticSeverity::Error
                && matches!(
                    &diagnostic.kind,
                    DiagnosticKind::DuplicateSymbol { key } if key == "Name.PropertyType"
                )
        }));
    }

    #[test]
    fn detects_unresolved_references() {
        let mut property =
            descriptor("Name.PropertyType", "Name", DescriptorKind::PropertyType, json_origin());
        push_reference(
            &mut property,
            SemanticReference::unresolved(ReferenceRole::ValueType, "MissingValueType"),
        );

        let mut model = SemanticModel::new();
        model.push_schema(schema(json_origin()));
        model.push_descriptor(property);

        let (symbols, diagnostics) = SymbolIndex::build(&mut model);

        assert_eq!(symbols.collect_unresolved_references(&model).len(), 1);
        assert!(diagnostics.iter().any(|diagnostic| {
            matches!(
                &diagnostic.kind,
                DiagnosticKind::UnresolvedReference { role: ReferenceRole::ValueType, target }
                    if target == "MissingValueType"
            )
        }));
    }

    #[test]
    fn detects_wrong_descriptor_kind_when_expected_kind_is_known() {
        let mut property =
            descriptor("Name.PropertyType", "Name", DescriptorKind::PropertyType, json_origin());
        push_reference(
            &mut property,
            SemanticReference::unresolved(ReferenceRole::ValueType, "Person.HolonType"),
        );

        let mut model = SemanticModel::new();
        model.push_schema(schema(json_origin()));
        model.push_descriptor(descriptor(
            "Person.HolonType",
            "Person",
            DescriptorKind::HolonType,
            json_origin(),
        ));
        model.push_descriptor(property);

        let (_, diagnostics) = SymbolIndex::build(&mut model);

        assert!(diagnostics.iter().any(|diagnostic| {
            matches!(
                &diagnostic.kind,
                DiagnosticKind::WrongDescriptorKind {
                    role: ReferenceRole::ValueType,
                    target,
                    actual: DescriptorKind::HolonType,
                    expected,
                } if target == "Person.HolonType"
                    && expected == &vec![DescriptorKind::ValueType, DescriptorKind::Enum]
            )
        }));
    }

    #[test]
    fn leaves_required_relationship_fields_to_the_validator_layer() {
        let mut model = SemanticModel::new();
        model.push_schema(schema(json_origin()));
        model.push_descriptor(descriptor(
            "Owns.RelationshipType",
            "Owns",
            DescriptorKind::RelationshipType,
            json_origin(),
        ));

        let (_, diagnostics) = SymbolIndex::build(&mut model);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn preserves_origin_metadata_on_symbols_and_diagnostics() {
        let mut model = SemanticModel::new();
        model.push_schema(schema(tdl_origin()));
        model.push_descriptor(descriptor(
            "Name.PropertyType",
            "Name",
            DescriptorKind::PropertyType,
            tdl_origin(),
        ));
        model.push_descriptor(descriptor(
            "Name.PropertyType",
            "Name",
            DescriptorKind::PropertyType,
            tdl_origin(),
        ));

        let (symbols, diagnostics) = SymbolIndex::build(&mut model);

        let schema = symbols.lookup_by_key("Test Schema").expect("schema symbol");
        assert_eq!(schema.origin.file_path, Some(PathBuf::from("schema.tdl")));
        assert_eq!(schema.origin.line, Some(3));
        assert_eq!(schema.origin.column, Some(7));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.origin.as_ref().and_then(|origin| origin.line) == Some(3)
        }));
    }

    #[test]
    fn semantic_comparison_ignores_origin_differences() {
        let mut json_model = SemanticModel::new();
        json_model.push_schema(schema(json_origin()));
        json_model.push_descriptor(descriptor(
            "MapStringValueType",
            "MapStringValueType",
            DescriptorKind::ValueType,
            json_origin(),
        ));
        let mut json_property =
            descriptor("Name.PropertyType", "Name", DescriptorKind::PropertyType, json_origin());
        push_reference(
            &mut json_property,
            SemanticReference::unresolved(ReferenceRole::ValueType, "MapStringValueType"),
        );
        json_model.push_descriptor(json_property);

        let mut tdl_model = SemanticModel::new();
        tdl_model.push_schema(schema(tdl_origin()));
        tdl_model.push_descriptor(descriptor(
            "MapStringValueType",
            "MapStringValueType",
            DescriptorKind::ValueType,
            tdl_origin(),
        ));
        let mut tdl_property =
            descriptor("Name.PropertyType", "Name", DescriptorKind::PropertyType, tdl_origin());
        push_reference(
            &mut tdl_property,
            SemanticReference::unresolved(ReferenceRole::ValueType, "MapStringValueType"),
        );
        tdl_model.push_descriptor(tdl_property);

        let (_, json_diagnostics) = SymbolIndex::build(&mut json_model);
        let (_, tdl_diagnostics) = SymbolIndex::build(&mut tdl_model);

        assert!(json_diagnostics.is_empty());
        assert!(tdl_diagnostics.is_empty());
        assert_eq!(json_model.comparable_signature(), tdl_model.comparable_signature());
    }
}
