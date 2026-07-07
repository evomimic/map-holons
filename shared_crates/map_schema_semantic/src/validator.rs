//! Schema-authoring validation over the Canonical Holon IR.
//!
//! This module owns source-neutral semantic checks that apply after lowering and reference
//! resolution. Source adapters remain responsible for syntax and source-format conveniences.

use crate::{
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
    diagnostics.extend(validate_projected_type_kinds(model));
    diagnostics.extend(validate_extends_graph(model, symbols, &descriptor_by_key));
    diagnostics.extend(validate_relationship_pairs(model, symbols, &descriptor_by_key));
    diagnostics.extend(validate_local_duplicates(model, symbols));
    diagnostics.extend(validate_effective_keys(model, symbols, &descriptor_by_key));

    diagnostics
}

/// Applies validation-only normalization that should not mutate emitted source artifacts.
pub fn normalize_validation_model(model: &mut SemanticModel) {
    for descriptor in &mut model.descriptors {
        if descriptor.kind != DescriptorKind::RelationshipType {
            continue;
        }
        if descriptor.min_cardinality.is_none() {
            descriptor.min_cardinality = Some(0);
        }
        if descriptor.max_cardinality.is_none() {
            descriptor.max_cardinality = Some(32_767);
        }
        if descriptor.relationship_flavor != Some(RelationshipFlavor::Inverse)
            && descriptor.deletion_semantic.is_none()
        {
            descriptor.deletion_semantic = Some("Allow".to_string());
        }
    }
}

/// Synthesizes missing inverse-pair references from the side that was authored explicitly.
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

fn validate_projected_type_kinds(model: &SemanticModel) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for descriptor in &model.descriptors {
        if is_bootstrap_descriptor(descriptor) {
            continue;
        }
        let Some(projected) = projected_type_kind(descriptor) else {
            continue;
        };
        let Some(authored) = descriptor
            .literal_properties
            .get("instance_type_kind")
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        if type_kind_family_for_text(authored) != type_kind_family_for_descriptor(descriptor) {
            diagnostics.push(Diagnostic::error(
                DiagnosticLayer::DescriptorKind,
                DiagnosticKind::InvalidProjectedTypeKind {
                    descriptor: descriptor.key.clone(),
                    actual: authored.to_string(),
                    expected: projected.to_string(),
                },
                Some(descriptor.origin.clone()),
            ));
        }
    }
    diagnostics
}

fn validate_extends_graph(
    model: &SemanticModel,
    symbols: &SymbolIndex,
    descriptor_by_key: &HashMap<&str, &TypeDescriptor>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for descriptor in &model.descriptors {
        let Some(extends) = &descriptor.extends else {
            continue;
        };
        let Some(target_symbol) = resolved_symbol(extends, symbols) else {
            continue;
        };
        let Some(parent) = descriptor_by_key.get(target_symbol.key.as_str()).copied() else {
            continue;
        };

        if descriptor.name.starts_with("Meta") || parent.name.starts_with("Meta") {
            continue;
        }
        if let (Some(actual), Some(expected)) =
            (type_kind_family_for_descriptor(descriptor), type_kind_family_for_descriptor(parent))
        {
            if !type_kind_families_compatible(actual, expected) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticLayer::SchemaAware,
                    DiagnosticKind::TypeKindMismatch {
                        descriptor: descriptor.key.clone(),
                        target: parent.key.clone(),
                        actual: projected_type_kind(descriptor)
                            .unwrap_or(actual.as_str())
                            .to_string(),
                        expected: projected_type_kind(parent)
                            .unwrap_or(expected.as_str())
                            .to_string(),
                    },
                    Some(descriptor.origin.clone()),
                ));
            }
        }

        if has_extends_cycle(descriptor, symbols, descriptor_by_key) {
            diagnostics.push(Diagnostic::error(
                DiagnosticLayer::SchemaAware,
                DiagnosticKind::InheritanceCycle {
                    descriptor: descriptor.key.clone(),
                    target: parent.key.clone(),
                },
                Some(descriptor.origin.clone()),
            ));
        }
    }
    diagnostics
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
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for descriptor in &model.descriptors {
        let Some(resolution) = resolve_effective_key_rule(descriptor, symbols, descriptor_by_key)
        else {
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

fn has_extends_cycle(
    descriptor: &TypeDescriptor,
    symbols: &SymbolIndex,
    descriptor_by_key: &HashMap<&str, &TypeDescriptor>,
) -> bool {
    let mut seen = HashSet::new();
    let mut current = descriptor.extends.as_ref();
    while let Some(reference) = current {
        let Some(symbol) = resolved_symbol(reference, symbols) else {
            return false;
        };
        if !seen.insert(symbol.key.clone()) {
            return true;
        }
        if symbol.key == descriptor.key {
            return true;
        }
        current = descriptor_by_key.get(symbol.key.as_str()).and_then(|next| next.extends.as_ref());
    }
    false
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

fn resolve_effective_key_rule<'a>(
    descriptor: &'a TypeDescriptor,
    symbols: &'a SymbolIndex,
    descriptor_by_key: &'a HashMap<&str, &TypeDescriptor>,
) -> Option<String> {
    if descriptor.extends.is_some() {
        let mut current = descriptor.extends.as_ref();
        let mut visited = HashSet::new();
        while let Some(reference) = current {
            let symbol = resolved_symbol(reference, symbols)?;
            if !visited.insert(symbol.key.clone()) {
                break;
            }
            let parent = descriptor_by_key.get(symbol.key.as_str())?;
            if let Some(rule) =
                parent.key_rule.as_ref().map(|reference| canonical_key_rule(&reference.target))
            {
                return Some(rule);
            }
            current = parent.extends.as_ref();
        }
    } else if let Some(rule) =
        descriptor.key_rule.as_ref().map(|reference| canonical_key_rule(&reference.target))
    {
        return Some(rule);
    }

    let described_by = describing_type_target(descriptor)?;
    let describing_symbol = symbols.lookup_reference_target(described_by)?;
    let mut current = descriptor_by_key.get(describing_symbol.key.as_str()).copied();
    let mut visited = HashSet::new();
    while let Some(next) = current {
        if !visited.insert(next.key.clone()) {
            break;
        }
        if let Some(rule) =
            next.key_rule.as_ref().map(|reference| canonical_key_rule(&reference.target))
        {
            return Some(rule);
        }
        current = next
            .extends
            .as_ref()
            .and_then(|reference| resolved_symbol(reference, symbols))
            .and_then(|symbol| descriptor_by_key.get(symbol.key.as_str()).copied());
    }
    None
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
        TYPE_KIND_RULE => projected_type_kind(descriptor)
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

fn describing_type_target(descriptor: &TypeDescriptor) -> Option<&'static str> {
    match descriptor.kind {
        DescriptorKind::TypeDescriptor | DescriptorKind::HolonType => Some("MetaHolonType"),
        DescriptorKind::PropertyType => Some("MetaPropertyType"),
        DescriptorKind::RelationshipType => {
            Some(if descriptor.relationship_flavor == Some(RelationshipFlavor::Inverse) {
                "MetaInverseRelationshipType"
            } else {
                "MetaDeclaredRelationshipType"
            })
        }
        DescriptorKind::ValueType | DescriptorKind::Enum => Some("MetaValueType"),
        DescriptorKind::EnumVariant | DescriptorKind::Schema => None,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeKindFamily {
    Holon,
    Property,
    Relationship,
    Value,
    EnumVariant,
}

impl TypeKindFamily {
    fn as_str(self) -> &'static str {
        match self {
            Self::Holon => "TypeKind.Holon",
            Self::Property => "TypeKind.Property",
            Self::Relationship => "TypeKind.Relationship",
            Self::Value => "TypeKind.Value",
            Self::EnumVariant => "TypeKind.EnumVariant",
        }
    }
}

fn type_kind_family_for_descriptor(descriptor: &TypeDescriptor) -> Option<TypeKindFamily> {
    match descriptor.kind {
        DescriptorKind::Schema => None,
        DescriptorKind::PropertyType => Some(TypeKindFamily::Property),
        DescriptorKind::RelationshipType => Some(TypeKindFamily::Relationship),
        DescriptorKind::ValueType | DescriptorKind::Enum => Some(TypeKindFamily::Value),
        DescriptorKind::EnumVariant => Some(TypeKindFamily::EnumVariant),
        DescriptorKind::TypeDescriptor | DescriptorKind::HolonType => {
            if descriptor.name == "PropertyType" || descriptor.name.ends_with("PropertyType") {
                Some(TypeKindFamily::Property)
            } else if descriptor.name == "ValueType" || descriptor.name.ends_with("ValueType") {
                Some(TypeKindFamily::Value)
            } else if descriptor.name == "DeclaredRelationshipType"
                || descriptor.name == "InverseRelationshipType"
                || descriptor.name == "MetaRelationshipType"
                || descriptor.name.ends_with("RelationshipType")
            {
                Some(TypeKindFamily::Relationship)
            } else {
                Some(TypeKindFamily::Holon)
            }
        }
    }
}

fn type_kind_family_for_text(text: &str) -> Option<TypeKindFamily> {
    if text.starts_with("TypeKind.Property") {
        Some(TypeKindFamily::Property)
    } else if text.starts_with("TypeKind.Relationship") {
        Some(TypeKindFamily::Relationship)
    } else if text.starts_with("TypeKind.Value") {
        Some(TypeKindFamily::Value)
    } else if text.starts_with("TypeKind.EnumVariant") {
        Some(TypeKindFamily::EnumVariant)
    } else if text.starts_with("TypeKind.Holon") {
        Some(TypeKindFamily::Holon)
    } else {
        None
    }
}

fn type_kind_families_compatible(actual: TypeKindFamily, expected: TypeKindFamily) -> bool {
    actual == expected
        || matches!(
            (actual, expected),
            (TypeKindFamily::EnumVariant, TypeKindFamily::Value)
                | (TypeKindFamily::Value, TypeKindFamily::EnumVariant)
        )
}

fn is_bootstrap_descriptor(descriptor: &TypeDescriptor) -> bool {
    descriptor.name.starts_with("Meta")
        || matches!(
            descriptor.name.as_str(),
            "TypeDescriptor"
                | "HolonType"
                | "ValueType"
                | "PropertyType"
                | "RelationshipType"
                | "DeclaredRelationshipType"
                | "InverseRelationshipType"
        )
}

fn projected_type_kind(descriptor: &TypeDescriptor) -> Option<&'static str> {
    match descriptor.kind {
        DescriptorKind::Schema => None,
        DescriptorKind::TypeDescriptor | DescriptorKind::HolonType => Some("TypeKind.Holon"),
        DescriptorKind::PropertyType => Some("TypeKind.Property"),
        DescriptorKind::RelationshipType => Some("TypeKind.Relationship"),
        DescriptorKind::Enum => Some("TypeKind.Value.Enum"),
        DescriptorKind::EnumVariant => Some("TypeKind.EnumVariant"),
        DescriptorKind::ValueType => Some(infer_value_kind(descriptor)),
    }
}

fn infer_value_kind(descriptor: &TypeDescriptor) -> &'static str {
    if descriptor.name == "MetaValueType" {
        return "TypeKind.Holon";
    }
    if descriptor.name == "ValueType" {
        return "TypeKind.Value";
    }
    if matches!(descriptor.name.as_str(), "EnumVariantValueType" | "MapEnumVariantValueType") {
        return "TypeKind.EnumVariant";
    }
    if descriptor.name.ends_with("StringValueType") {
        return "TypeKind.Value.String";
    }
    if descriptor.name.ends_with("IntegerValueType") {
        return "TypeKind.Value.Integer";
    }
    if descriptor.name.ends_with("BooleanValueType") {
        return "TypeKind.Value.Boolean";
    }
    if descriptor.name.ends_with("BytesValueType") || descriptor.name == "HolonIdValueType" {
        return "TypeKind.Value.Bytes";
    }
    if descriptor.name.ends_with("ValueArrayType")
        || descriptor.name.ends_with("ValueArrayValueType")
    {
        return "TypeKind.Value.Array";
    }
    if descriptor.name.ends_with("EnumValueType") {
        return "TypeKind.Value.Enum";
    }
    match descriptor.extends.as_ref().map(|reference| reference.target.as_str()) {
        Some("MetaTypeDescriptor") | Some("MetaValueType") => "TypeKind.Holon",
        Some("StringValueType") | Some("MapStringValueType") => "TypeKind.Value.String",
        Some("IntegerValueType") | Some("MapIntegerValueType") => "TypeKind.Value.Integer",
        Some("BooleanValueType") | Some("MapBooleanValueType") => "TypeKind.Value.Boolean",
        Some("BytesValueType") | Some("MapBytesValueType") | Some("HolonIdValueType") => {
            "TypeKind.Value.Bytes"
        }
        Some("ValueArrayValueType") | Some("MapValueArrayType") => "TypeKind.Value.Array",
        Some("EnumValueType") | Some("MapEnumValueType") => "TypeKind.Value.Enum",
        Some("EnumVariantValueType") | Some("MapEnumVariantValueType") => "TypeKind.EnumVariant",
        _ => "TypeKind.Value.String",
    }
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
}
