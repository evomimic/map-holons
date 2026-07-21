//! Schema-authoring validation over the Canonical Holon IR.
//!
//! This module owns source-neutral semantic checks that apply after lowering and reference
//! resolution. Source adapters remain responsible for syntax and source-format conveniences.

use descriptor_semantics::{ancestors, describing_type, effective_holon_key_rule, DescriptorGraph};

use crate::{
    descriptor_conformance::validate_canonical_model_values,
    descriptor_graph::CanonicalDescriptorGraph,
    diagnostics::{Diagnostic, DiagnosticKind, DiagnosticLayer},
    schema_index::{Symbol, SymbolIndex},
    schema_ir::{
        DescriptorKind, ReferenceRole, RelationshipFlavor, SemanticModel, SemanticReference,
        TypeDescriptor,
    },
};
use std::collections::{HashMap, HashSet};

const TYPE_NAME_RULE: &str = "TypeNameRule.KeyRuleType";
const SCHEMA_NAME_RULE: &str = "SchemaNameRule.KeyRuleType";
const TYPE_KIND_RULE: &str = "TypeKindRule.KeyRuleType";
const ENUM_VARIANT_RULE: &str = "EnumVariantRule.KeyRuleType";
const RELATIONSHIP_RULE: &str = "RelationshipRule.KeyRuleType";
const EXTENDED_TYPE_RULE: &str = "ExtendedTypeRule.KeyRuleType";
const NONE_RULE: &str = "NoneRule.KeyRuleType";

/// Validates one semantic model after reference resolution.
pub fn validate_model(model: &SemanticModel, symbols: &SymbolIndex) -> Vec<Diagnostic> {
    let descriptor_by_key = model
        .descriptors
        .iter()
        .map(|descriptor| (descriptor.key.as_str(), descriptor))
        .collect::<HashMap<_, _>>();
    let mut diagnostics = Vec::new();

    diagnostics.extend(validate_required_slots(model));
    if let Ok(graph) = CanonicalDescriptorGraph::new(model, symbols) {
        diagnostics.extend(validate_canonical_model_values(&graph));
        diagnostics.extend(validate_inheritance_graph(&graph));
        diagnostics.extend(validate_effective_keys(model, symbols, &descriptor_by_key, &graph));
    }
    diagnostics.extend(validate_relationship_pairs(model, symbols, &descriptor_by_key));
    diagnostics.extend(validate_local_duplicates(model, symbols));

    diagnostics
}

/// Completes inverse-pair references from whichever side authored the pair.
///
/// Source adapters may apply this to a validation clone when their representation serializes only
/// one direction. The authored model remains unchanged for faithful round trips.
pub fn normalize_relationship_pairs(model: &mut SemanticModel, symbols: &SymbolIndex) {
    let descriptor_indexes = model
        .descriptors
        .iter()
        .enumerate()
        .map(|(index, descriptor)| (descriptor.key.clone(), index))
        .collect::<HashMap<_, _>>();

    for index in 0..model.descriptors.len() {
        if model.descriptors[index].kind != DescriptorKind::RelationshipType {
            continue;
        }

        let current_key = model.descriptors[index].key.clone();
        let current_symbol_id = symbols.lookup_by_key(&current_key).map(|symbol| symbol.id);

        if let Some(has_inverse) = model.descriptors[index].has_inverse.clone() {
            if let Some(target_symbol) = resolved_symbol(&has_inverse, symbols) {
                if let Some(target_index) = descriptor_indexes.get(&target_symbol.key).copied() {
                    if model.descriptors[target_index].inverse_of.is_none() {
                        model.descriptors[target_index].inverse_of = Some(SemanticReference {
                            role: ReferenceRole::InverseOf,
                            target: current_key.clone(),
                            resolved: current_symbol_id,
                        });
                    }
                }
            }
        }

        if let Some(inverse_of) = model.descriptors[index].inverse_of.clone() {
            if let Some(target_symbol) = resolved_symbol(&inverse_of, symbols) {
                if let Some(target_index) = descriptor_indexes.get(&target_symbol.key).copied() {
                    if model.descriptors[target_index].has_inverse.is_none() {
                        model.descriptors[target_index].has_inverse = Some(SemanticReference {
                            role: ReferenceRole::HasInverse,
                            target: current_key.clone(),
                            resolved: current_symbol_id,
                        });
                    }
                }
            }
        }
    }
}

fn validate_required_slots(model: &SemanticModel) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for descriptor in &model.descriptors {
        let mut required_fields = Vec::new();

        required_fields.push("ComponentOf");

        if requires_extends(descriptor) {
            required_fields.push("Extends");
        }

        match descriptor.kind {
            DescriptorKind::PropertyType => required_fields.push("ValueType"),
            DescriptorKind::RelationshipType => {
                required_fields.push("SourceType");
                required_fields.push("TargetType");
                required_fields.push("min_cardinality");
                required_fields.push("max_cardinality");
                if descriptor.relationship_flavor != Some(RelationshipFlavor::Inverse) {
                    required_fields.push("deletion_semantic");
                    required_fields.push("HasInverse");
                } else {
                    required_fields.push("InverseOf");
                }
            }
            DescriptorKind::Enum => {
                if descriptor.variants.is_empty()
                    && !descriptor.is_abstract
                    && !descriptor.name.ends_with("ValueType")
                {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticLayer::SchemaAware,
                        DiagnosticKind::MissingRequiredField {
                            descriptor: descriptor.key.clone(),
                            field: "Variants".to_string(),
                        },
                        Some(descriptor.origin.clone()),
                    ));
                }
            }
            DescriptorKind::EnumVariant => {
                if !descriptor.is_abstract && !descriptor.name.ends_with("ValueType") {
                    required_fields.push("VariantOf");
                }
            }
            DescriptorKind::Schema
            | DescriptorKind::TypeDescriptor
            | DescriptorKind::HolonType
            | DescriptorKind::ValueType => {}
        }

        for field in required_fields {
            let present = match field {
                "ComponentOf" => descriptor.component_of.is_some(),
                "Extends" => descriptor.extends.is_some(),
                "ValueType" => descriptor.value_type.is_some(),
                "SourceType" => descriptor.source_type.is_some(),
                "TargetType" => descriptor.target_type.is_some(),
                "HasInverse" => descriptor.has_inverse.is_some(),
                "InverseOf" => descriptor.inverse_of.is_some(),
                "VariantOf" => descriptor.variant_of.is_some(),
                "min_cardinality" => descriptor.min_cardinality.is_some(),
                "max_cardinality" => descriptor.max_cardinality.is_some(),
                "deletion_semantic" => descriptor.deletion_semantic.is_some(),
                _ => true,
            };

            if !present {
                diagnostics.push(Diagnostic::error(
                    DiagnosticLayer::SchemaAware,
                    DiagnosticKind::MissingRequiredField {
                        descriptor: descriptor.key.clone(),
                        field: field.to_string(),
                    },
                    Some(descriptor.origin.clone()),
                ));
            }
        }

        if let (Some(min), Some(max)) = (descriptor.min_cardinality, descriptor.max_cardinality) {
            if min > max {
                diagnostics.push(Diagnostic::error(
                    DiagnosticLayer::SchemaAware,
                    DiagnosticKind::InvalidCardinalityBounds {
                        descriptor: descriptor.key.clone(),
                        min,
                        max,
                    },
                    Some(descriptor.origin.clone()),
                ));
            }
        }
    }
    diagnostics
}

fn validate_inheritance_graph(graph: &CanonicalDescriptorGraph) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for node in graph.nodes() {
        let described_by = match describing_type(graph, &node) {
            Ok(described_by) => described_by,
            Err(error) => {
                diagnostics.push(graph.diagnostic(error));
                continue;
            }
        };
        let source_is_type = match graph.is_type_descriptor(&described_by) {
            Ok(is_type) => is_type,
            Err(error) => {
                diagnostics.push(
                    graph.diagnostic(descriptor_semantics::DescriptorSemanticsError::Access(error)),
                );
                continue;
            }
        };
        let extends_targets = match graph.extends_targets(&node) {
            Ok(targets) => targets,
            Err(error) => {
                diagnostics.push(
                    graph.diagnostic(descriptor_semantics::DescriptorSemanticsError::Access(error)),
                );
                continue;
            }
        };

        if !extends_targets.is_empty() && !source_is_type {
            diagnostics.push(extends_endpoint_not_type(graph, node, node));
        }

        if let Err(error) = ancestors(graph, &node) {
            diagnostics.push(graph.diagnostic(error));
        }

        for target in extends_targets {
            let target_is_type = describing_type(graph, &target).and_then(|target_descriptor| {
                graph
                    .is_type_descriptor(&target_descriptor)
                    .map_err(descriptor_semantics::DescriptorSemanticsError::Access)
            });
            match target_is_type {
                Ok(false) => diagnostics.push(extends_endpoint_not_type(graph, node, target)),
                Err(error) => {
                    diagnostics.push(graph.diagnostic(error));
                    continue;
                }
                Ok(true) => {}
            }
        }
    }
    diagnostics
}

fn extends_endpoint_not_type(
    graph: &CanonicalDescriptorGraph,
    source: crate::CanonicalNodeId,
    endpoint: crate::CanonicalNodeId,
) -> Diagnostic {
    Diagnostic::error(
        DiagnosticLayer::SchemaAware,
        DiagnosticKind::ExtendsEndpointNotType {
            descriptor: graph.key(source).unwrap_or("<unknown>").to_string(),
            endpoint: graph.key(endpoint).unwrap_or("<unknown>").to_string(),
        },
        graph.holon(source).map(|holon| holon.origin.clone()),
    )
}

fn validate_relationship_pairs(
    model: &SemanticModel,
    symbols: &SymbolIndex,
    descriptor_by_key: &HashMap<&str, &TypeDescriptor>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut declared_to_inverses: HashMap<String, Vec<String>> = HashMap::new();
    let mut inverse_to_declareds: HashMap<String, Vec<String>> = HashMap::new();

    for descriptor in &model.descriptors {
        if descriptor.kind != DescriptorKind::RelationshipType {
            continue;
        }
        if let Some(has_inverse) = &descriptor.has_inverse {
            inverse_to_declareds
                .entry(has_inverse.target.clone())
                .or_default()
                .push(descriptor.key.clone());
        }
        if let Some(inverse_of) = &descriptor.inverse_of {
            declared_to_inverses
                .entry(inverse_of.target.clone())
                .or_default()
                .push(descriptor.key.clone());
        }
    }

    for descriptor in &model.descriptors {
        if descriptor.kind != DescriptorKind::RelationshipType {
            continue;
        }

        match descriptor.relationship_flavor.unwrap_or(RelationshipFlavor::Declared) {
            RelationshipFlavor::Declared => {
                let Some(has_inverse) = &descriptor.has_inverse else {
                    continue;
                };
                if inverse_to_declareds.get(&has_inverse.target).map_or(0, Vec::len) > 1 {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticLayer::SchemaAware,
                        DiagnosticKind::DuplicateInverseRelationship {
                            descriptor: descriptor.key.clone(),
                            inverse: has_inverse.target.clone(),
                        },
                        Some(descriptor.origin.clone()),
                    ));
                }

                let Some(symbol) = resolved_symbol(has_inverse, symbols) else {
                    continue;
                };
                let Some(inverse_descriptor) = descriptor_by_key.get(symbol.key.as_str()).copied()
                else {
                    continue;
                };
                if inverse_descriptor.relationship_flavor != Some(RelationshipFlavor::Inverse) {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticLayer::SchemaAware,
                        DiagnosticKind::WrongRelationshipFlavor {
                            role: ReferenceRole::HasInverse,
                            descriptor: descriptor.key.clone(),
                            target: inverse_descriptor.key.clone(),
                            actual: relationship_flavor_label(
                                inverse_descriptor.relationship_flavor,
                            ),
                            expected: "inverse".to_string(),
                        },
                        Some(descriptor.origin.clone()),
                    ));
                }
                if let Some(back_reference) =
                    inverse_descriptor.inverse_of.as_ref().map(|reference| reference.target.clone())
                {
                    if back_reference != descriptor.key && back_reference != descriptor.name {
                        diagnostics.push(Diagnostic::error(
                            DiagnosticLayer::SchemaAware,
                            DiagnosticKind::InverseRelationshipMismatch {
                                descriptor: descriptor.key.clone(),
                                inverse: inverse_descriptor.key.clone(),
                                expected: descriptor.key.clone(),
                            },
                            Some(descriptor.origin.clone()),
                        ));
                    }
                }
            }
            RelationshipFlavor::Inverse => {
                let Some(inverse_of) = &descriptor.inverse_of else {
                    continue;
                };
                if declared_to_inverses.get(&inverse_of.target).map_or(0, Vec::len) > 1 {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticLayer::SchemaAware,
                        DiagnosticKind::DuplicateInverseRelationship {
                            descriptor: descriptor.key.clone(),
                            inverse: inverse_of.target.clone(),
                        },
                        Some(descriptor.origin.clone()),
                    ));
                }

                let Some(symbol) = resolved_symbol(inverse_of, symbols) else {
                    continue;
                };
                let Some(declared_descriptor) = descriptor_by_key.get(symbol.key.as_str()).copied()
                else {
                    continue;
                };
                if declared_descriptor.relationship_flavor == Some(RelationshipFlavor::Inverse) {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticLayer::SchemaAware,
                        DiagnosticKind::WrongRelationshipFlavor {
                            role: ReferenceRole::InverseOf,
                            descriptor: descriptor.key.clone(),
                            target: declared_descriptor.key.clone(),
                            actual: relationship_flavor_label(
                                declared_descriptor.relationship_flavor,
                            ),
                            expected: "declared".to_string(),
                        },
                        Some(descriptor.origin.clone()),
                    ));
                }
                if let Some(back_reference) = declared_descriptor
                    .has_inverse
                    .as_ref()
                    .map(|reference| reference.target.clone())
                {
                    if back_reference != descriptor.key && back_reference != descriptor.name {
                        diagnostics.push(Diagnostic::error(
                            DiagnosticLayer::SchemaAware,
                            DiagnosticKind::InverseRelationshipMismatch {
                                descriptor: descriptor.key.clone(),
                                inverse: declared_descriptor.key.clone(),
                                expected: descriptor.key.clone(),
                            },
                            Some(descriptor.origin.clone()),
                        ));
                    }
                }
            }
        }
    }

    diagnostics
}

fn validate_local_duplicates(model: &SemanticModel, symbols: &SymbolIndex) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for descriptor in &model.descriptors {
        diagnostics.extend(find_duplicate_members(
            descriptor,
            &descriptor.instance_properties,
            ReferenceRole::InstanceProperty,
            symbols,
        ));
        diagnostics.extend(find_duplicate_members(
            descriptor,
            &descriptor.instance_relationships,
            ReferenceRole::InstanceRelationship,
            symbols,
        ));
    }
    diagnostics
}

fn validate_effective_keys(
    model: &SemanticModel,
    symbols: &SymbolIndex,
    descriptor_by_key: &HashMap<&str, &TypeDescriptor>,
    graph: &CanonicalDescriptorGraph,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for descriptor in &model.descriptors {
        let Some(node) = graph.node_by_key(&descriptor.key) else {
            continue;
        };
        let resolution = match effective_holon_key_rule(graph, &node, &"UsesKeyRule".to_string()) {
            Ok(Some(rule)) => graph.key(rule).map(canonical_key_rule),
            Ok(None) => None,
            Err(error) => {
                diagnostics.push(graph.diagnostic(error));
                continue;
            }
        };
        let Some(resolution) = resolution else {
            diagnostics.push(Diagnostic::error(
                DiagnosticLayer::SchemaAware,
                DiagnosticKind::MissingEffectiveKeyRule { descriptor: descriptor.key.clone() },
                Some(descriptor.origin.clone()),
            ));
            continue;
        };

        let generated = match generate_key(descriptor, &resolution, symbols, descriptor_by_key) {
            Ok(generated) => generated,
            Err(kind) => {
                diagnostics.push(Diagnostic::error(
                    DiagnosticLayer::SchemaAware,
                    kind,
                    Some(descriptor.origin.clone()),
                ));
                continue;
            }
        };

        if generated != descriptor.key {
            diagnostics.push(Diagnostic::error(
                DiagnosticLayer::SchemaAware,
                DiagnosticKind::AuthoredKeyMismatch {
                    descriptor: descriptor.key.clone(),
                    expected: generated,
                    actual: descriptor.key.clone(),
                },
                Some(descriptor.origin.clone()),
            ));
        }
    }
    diagnostics
}

fn requires_extends(descriptor: &TypeDescriptor) -> bool {
    !(descriptor.kind == DescriptorKind::Schema
        || (descriptor.kind == DescriptorKind::HolonType
            && descriptor.name == "MetaTypeDescriptor"))
}

fn find_duplicate_members(
    descriptor: &TypeDescriptor,
    references: &[SemanticReference],
    role: ReferenceRole,
    symbols: &SymbolIndex,
) -> Vec<Diagnostic> {
    let mut seen = HashSet::new();
    let mut diagnostics = Vec::new();
    for reference in references {
        let name = resolved_symbol(reference, symbols)
            .map(|symbol| symbol.name.clone())
            .unwrap_or_else(|| reference.target.clone());
        if !seen.insert(name.clone()) {
            diagnostics.push(Diagnostic::error(
                DiagnosticLayer::SchemaAware,
                DiagnosticKind::DuplicateLocalMember {
                    descriptor: descriptor.key.clone(),
                    role,
                    name,
                },
                Some(descriptor.origin.clone()),
            ));
        }
    }
    diagnostics
}

fn generate_key(
    descriptor: &TypeDescriptor,
    key_rule: &str,
    symbols: &SymbolIndex,
    descriptor_by_key: &HashMap<&str, &TypeDescriptor>,
) -> Result<String, DiagnosticKind> {
    match key_rule {
        TYPE_NAME_RULE => Ok(descriptor.name.clone()),
        SCHEMA_NAME_RULE => Ok(descriptor.owning_schema.clone()),
        TYPE_KIND_RULE => descriptor
            .instance_type_kind
            .as_deref()
            .map(|kind| format!("{}.{}", descriptor.name, kind))
            .ok_or_else(|| DiagnosticKind::MissingKeyRuleInput {
                descriptor: descriptor.key.clone(),
                key_rule: key_rule.to_string(),
                field: "instance_type_kind".to_string(),
            }),
        ENUM_VARIANT_RULE => {
            let Some(parent) = descriptor.variant_of.as_ref() else {
                return Err(DiagnosticKind::MissingKeyRuleInput {
                    descriptor: descriptor.key.clone(),
                    key_rule: key_rule.to_string(),
                    field: "VariantOf".to_string(),
                });
            };
            let parent_key = resolved_symbol(parent, symbols)
                .map(|symbol| symbol.key.clone())
                .unwrap_or_else(|| parent.target.clone());
            Ok(format!("{}.{}", parent_key, descriptor.name))
        }
        RELATIONSHIP_RULE => {
            let Some(source) = descriptor.source_type.as_ref() else {
                return Err(DiagnosticKind::MissingKeyRuleInput {
                    descriptor: descriptor.key.clone(),
                    key_rule: key_rule.to_string(),
                    field: "SourceType".to_string(),
                });
            };
            let Some(target) = descriptor.target_type.as_ref() else {
                return Err(DiagnosticKind::MissingKeyRuleInput {
                    descriptor: descriptor.key.clone(),
                    key_rule: key_rule.to_string(),
                    field: "TargetType".to_string(),
                });
            };
            let source_key = resolved_symbol(source, symbols)
                .map(|symbol| symbol.key.clone())
                .unwrap_or_else(|| source.target.clone());
            let target_key = resolved_symbol(target, symbols)
                .map(|symbol| symbol.key.clone())
                .unwrap_or_else(|| target.target.clone());
            Ok(format!("({source_key})-[{}]->({target_key})", descriptor.name))
        }
        EXTENDED_TYPE_RULE => {
            let Some(parent) = descriptor.extends.as_ref() else {
                return Err(DiagnosticKind::MissingKeyRuleInput {
                    descriptor: descriptor.key.clone(),
                    key_rule: key_rule.to_string(),
                    field: "Extends".to_string(),
                });
            };
            let parent_name = resolved_symbol(parent, symbols)
                .and_then(|symbol| descriptor_by_key.get(symbol.key.as_str()).copied())
                .map(|descriptor| descriptor.name.clone())
                .unwrap_or_else(|| parent.target.clone());
            Ok(format!("{}.{}", descriptor.name, parent_name))
        }
        NONE_RULE => Ok(String::new()),
        _ => Err(DiagnosticKind::UnsupportedKeyRule {
            descriptor: descriptor.key.clone(),
            key_rule: key_rule.to_string(),
        }),
    }
}

fn canonical_key_rule(target: &str) -> String {
    match target {
        "TypeNameRule" | TYPE_NAME_RULE => TYPE_NAME_RULE.to_string(),
        "SchemaNameRule" | SCHEMA_NAME_RULE => SCHEMA_NAME_RULE.to_string(),
        "TypeKindRule" | TYPE_KIND_RULE => TYPE_KIND_RULE.to_string(),
        "EnumVariantRule" | "EnumVariantRule.FormatRule" | ENUM_VARIANT_RULE => {
            ENUM_VARIANT_RULE.to_string()
        }
        "RelationshipRule" | RELATIONSHIP_RULE => RELATIONSHIP_RULE.to_string(),
        "ExtendedTypeRule" | EXTENDED_TYPE_RULE => EXTENDED_TYPE_RULE.to_string(),
        "NoneRule" | NONE_RULE => NONE_RULE.to_string(),
        other => other.to_string(),
    }
}

fn relationship_flavor_label(flavor: Option<RelationshipFlavor>) -> String {
    match flavor {
        Some(RelationshipFlavor::Declared) => "declared".to_string(),
        Some(RelationshipFlavor::Inverse) => "inverse".to_string(),
        None => "unspecified".to_string(),
    }
}

fn resolved_symbol<'a>(
    reference: &SemanticReference,
    symbols: &'a SymbolIndex,
) -> Option<&'a Symbol> {
    reference.resolved.and_then(|id| symbols.lookup_by_id(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_ir::{Origin, SourceKind};

    fn origin() -> Origin {
        Origin::new(SourceKind::Generated)
    }

    fn schema_model() -> SemanticModel {
        let mut model = SemanticModel::new();
        model.push_schema(crate::schema_ir::Schema {
            name: "Test Schema".to_string(),
            key: "Test Schema".to_string(),
            origin: origin(),
            described_by: Vec::new(),
            dependencies: Vec::new(),
            literal_properties: crate::literal_value::LiteralObject::new(),
            literal_relationships: Vec::new(),
            header: None,
            allows_additional_properties: false,
            allows_additional_relationships: false,
        });
        model
    }

    #[test]
    fn detects_invalid_cardinality_bounds() {
        let mut model = schema_model();
        let mut descriptor = TypeDescriptor::new(
            "Owns.RelationshipType",
            "Owns",
            DescriptorKind::RelationshipType,
            "Test Schema",
            origin(),
        );
        descriptor.component_of =
            Some(SemanticReference::unresolved(ReferenceRole::ComponentOf, "Test Schema"));
        descriptor.extends =
            Some(SemanticReference::unresolved(ReferenceRole::Extends, "DeclaredRelationshipType"));
        descriptor.relationship_flavor = Some(RelationshipFlavor::Declared);
        descriptor.min_cardinality = Some(3);
        descriptor.max_cardinality = Some(1);
        model.push_descriptor(descriptor);

        let (symbols, _) = SymbolIndex::build(&mut model);
        let diagnostics = validate_model(&model, &symbols);
        assert!(diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.kind,
            DiagnosticKind::InvalidCardinalityBounds { .. }
        )));
    }

    #[test]
    fn detects_duplicate_instance_property_names() {
        let mut model = schema_model();
        let value = TypeDescriptor::new(
            "MapStringValueType",
            "MapStringValueType",
            DescriptorKind::ValueType,
            "Test Schema",
            origin(),
        );
        let mut property = TypeDescriptor::new(
            "Name.PropertyType",
            "Name",
            DescriptorKind::PropertyType,
            "Test Schema",
            origin(),
        );
        property.component_of =
            Some(SemanticReference::unresolved(ReferenceRole::ComponentOf, "Test Schema"));
        property.extends =
            Some(SemanticReference::unresolved(ReferenceRole::Extends, "PropertyType"));
        property.value_type =
            Some(SemanticReference::unresolved(ReferenceRole::ValueType, "MapStringValueType"));
        let mut holon = TypeDescriptor::new(
            "Person.HolonType",
            "Person",
            DescriptorKind::HolonType,
            "Test Schema",
            origin(),
        );
        holon.component_of =
            Some(SemanticReference::unresolved(ReferenceRole::ComponentOf, "Test Schema"));
        holon.extends = Some(SemanticReference::unresolved(ReferenceRole::Extends, "HolonType"));
        holon.instance_properties = vec![
            SemanticReference::unresolved(ReferenceRole::InstanceProperty, "Name.PropertyType"),
            SemanticReference::unresolved(ReferenceRole::InstanceProperty, "Name.PropertyType"),
        ];
        model.push_descriptor(value);
        model.push_descriptor(property);
        model.push_descriptor(holon);

        let (symbols, _) = SymbolIndex::build(&mut model);
        let diagnostics = validate_model(&model, &symbols);
        assert!(diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.kind,
            DiagnosticKind::DuplicateLocalMember { role: ReferenceRole::InstanceProperty, .. }
        )));
    }

    fn inheritance_model(
        child_type_kind: &str,
        parent_type_kind: &str,
        parent_described_by: &str,
    ) -> SemanticModel {
        let mut model = SemanticModel::new();
        let mut type_descriptor = TypeDescriptor::new(
            "TypeDescriptor.HolonType",
            "TypeDescriptor",
            DescriptorKind::TypeDescriptor,
            "Test Schema",
            origin(),
        );
        type_descriptor.described_by.push(SemanticReference::unresolved(
            ReferenceRole::DescribedBy,
            "TypeDescriptor.HolonType",
        ));
        type_descriptor.instance_type_kind = Some("TypeKind.Holon".to_string());

        let mut ordinary_descriptor = TypeDescriptor::new(
            "OrdinaryDescriptor.HolonType",
            "OrdinaryDescriptor",
            DescriptorKind::HolonType,
            "Test Schema",
            origin(),
        );
        ordinary_descriptor.described_by.push(SemanticReference::unresolved(
            ReferenceRole::DescribedBy,
            "TypeDescriptor.HolonType",
        ));

        let mut parent = TypeDescriptor::new(
            "Parent.HolonType",
            "Parent",
            DescriptorKind::HolonType,
            "Test Schema",
            origin(),
        );
        parent
            .described_by
            .push(SemanticReference::unresolved(ReferenceRole::DescribedBy, parent_described_by));
        parent.instance_type_kind = Some(parent_type_kind.to_string());

        let mut child = TypeDescriptor::new(
            "Child.HolonType",
            "Child",
            DescriptorKind::HolonType,
            "Test Schema",
            origin(),
        );
        child.described_by.push(SemanticReference::unresolved(
            ReferenceRole::DescribedBy,
            "TypeDescriptor.HolonType",
        ));
        child.instance_type_kind = Some(child_type_kind.to_string());
        child.extends =
            Some(SemanticReference::unresolved(ReferenceRole::Extends, "Parent.HolonType"));

        model.push_descriptor(type_descriptor);
        model.push_descriptor(ordinary_descriptor);
        model.push_descriptor(parent);
        model.push_descriptor(child);
        model
    }

    #[test]
    fn rejects_extends_target_that_is_not_a_type() {
        let mut model =
            inheritance_model("TypeKind.Holon", "TypeKind.Holon", "OrdinaryDescriptor.HolonType");
        let (symbols, _) = SymbolIndex::build(&mut model);
        let graph = CanonicalDescriptorGraph::new(&model, &symbols).expect("canonical graph");

        let diagnostics = validate_inheritance_graph(&graph);

        assert!(diagnostics.iter().any(|diagnostic| matches!(
            &diagnostic.kind,
            DiagnosticKind::ExtendsEndpointNotType { descriptor, endpoint }
                if descriptor == "Child.HolonType" && endpoint == "Parent.HolonType"
        )));
    }
}
