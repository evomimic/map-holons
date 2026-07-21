//! Descriptor-driven conformance over Canonical Holon IR.
//!
//! This module is the representation adapter: it resolves effective declarations from canonical
//! holons, delegates policy evaluation to `descriptor_semantics`, and projects neutral violations
//! into shared diagnostics. It intentionally contains no property-specific validation rules.

use descriptor_semantics::{
    collect_named_members_from_lineage, compose_restrictive_boolean, describing_type,
    effective_descriptor_lineage, validate_holon_conformance, value_policy_for_type_kind,
    ConformanceValue, ConformanceViolation, DescriptorGraph, DescriptorSemanticsError,
    HolonConformance, PropertyDeclaration, PropertyValue, RelationshipDeclaration,
    RelationshipValue, ValuePolicy,
};

use crate::{
    CanonicalDescriptorGraph, CanonicalGraphError, CanonicalNodeId, Diagnostic, DiagnosticKind,
    DiagnosticLayer, LiteralValue,
};

/// Runs descriptor-driven conformance for every holon exposed by the canonical graph.
///
/// Callers may retain legacy/model-wide passes alongside this result while migrating checks; this
/// pass itself applies one descriptor-driven path to schema, descriptor, and ordinary holons.
pub fn validate_canonical_model_conformance(graph: &CanonicalDescriptorGraph) -> Vec<Diagnostic> {
    graph
        .nodes()
        .filter(|node| !graph.is_schema(*node))
        .flat_map(|node| validate_canonical_holon_conformance(graph, node))
        .collect()
}

/// Runs the production-ready value-policy subset of descriptor conformance.
///
/// This staged entry point validates primitive shape and enum membership for every property whose
/// effective descriptor supplies a value policy. Required/member/cardinality diagnostics remain
/// available through [`validate_canonical_model_conformance`] while the Core Schema corpus is
/// brought into strict conformance.
pub fn validate_canonical_model_values(graph: &CanonicalDescriptorGraph) -> Vec<Diagnostic> {
    validate_canonical_model_conformance(graph)
        .into_iter()
        .filter(|diagnostic| {
            matches!(diagnostic.kind, DiagnosticKind::InvalidConformanceValue { .. })
        })
        .collect()
}

/// Validates one canonical holon using declarations resolved from its effective descriptor graph.
pub fn validate_canonical_holon_conformance(
    graph: &CanonicalDescriptorGraph,
    node: CanonicalNodeId,
) -> Vec<Diagnostic> {
    let input = match conformance_input(graph, node) {
        Ok(input) => input,
        Err(error) => return vec![conformance_build_diagnostic(graph, node, error)],
    };

    let holon = graph.key(node).unwrap_or("<unknown>").to_string();
    let origin = graph.holon(node).map(|holon| holon.origin.clone());
    validate_holon_conformance(&input)
        .into_iter()
        .map(|violation| project_violation(&holon, origin.clone(), violation))
        .collect()
}

/// Resolves Boolean property declarations effective for one canonical holon.
///
/// Source adapters use this view to lower source-level Boolean default conventions into explicit
/// canonical values without copying inheritance or value-type classification rules.
pub fn effective_boolean_property_names(
    graph: &CanonicalDescriptorGraph,
    node: CanonicalNodeId,
) -> Result<Vec<String>, Diagnostic> {
    effective_property_declarations(graph, node)
        .map(|declarations| {
            declarations
                .into_iter()
                .filter(|declaration| declaration.value_policy == ValuePolicy::Boolean)
                .map(|declaration| declaration.name)
                .collect()
        })
        .map_err(|error| conformance_build_diagnostic(graph, node, error))
}

#[derive(Debug)]
enum ConformanceBuildError {
    Graph(DescriptorSemanticsError<CanonicalGraphError, CanonicalNodeId>),
    InvalidDescriptor { descriptor: String, message: String },
}

fn conformance_build_diagnostic(
    graph: &CanonicalDescriptorGraph,
    node: CanonicalNodeId,
    error: ConformanceBuildError,
) -> Diagnostic {
    match error {
        ConformanceBuildError::Graph(error) => graph.diagnostic(error),
        ConformanceBuildError::InvalidDescriptor { descriptor, message } => Diagnostic::error(
            DiagnosticLayer::SchemaAware,
            DiagnosticKind::DescriptorGraphAccess {
                holon: graph.key(node).unwrap_or("<unknown>").to_string(),
                relationship: "descriptor conformance".to_string(),
                target: Some(descriptor),
                message,
            },
            graph.holon(node).map(|holon| holon.origin.clone()),
        ),
    }
}

fn conformance_input(
    graph: &CanonicalDescriptorGraph,
    node: CanonicalNodeId,
) -> Result<HolonConformance, ConformanceBuildError> {
    let holon = graph.holon(node).ok_or_else(|| ConformanceBuildError::InvalidDescriptor {
        descriptor: format!("{node:?}"),
        message: "canonical holon is absent".to_string(),
    })?;
    describing_type(graph, &node).map_err(ConformanceBuildError::Graph)?;
    let lineage =
        effective_descriptor_lineage(graph, &node).map_err(ConformanceBuildError::Graph)?;
    let relationship_nodes = effective_named_members(
        graph,
        &lineage,
        "InstanceRelationships",
        "relationship",
        &["relationship_name", "type_name"],
    )?;

    let property_declarations = effective_property_declarations(graph, node)?;
    let relationship_declarations = relationship_nodes
        .into_iter()
        .map(|descriptor| relationship_declaration(graph, descriptor))
        .collect::<Result<Vec<_>, _>>()?;

    let mut properties = holon
        .properties
        .iter()
        .map(|(name, value)| PropertyValue { name: name.clone(), value: conformance_value(value) })
        .collect::<Vec<_>>();
    align_property_names(&mut properties, &property_declarations);
    let relationships = holon
        .relationships
        .iter()
        .map(|relationship| RelationshipValue {
            name: relationship.name.clone(),
            target_count: relationship.targets.len(),
        })
        .collect::<Vec<_>>();

    Ok(HolonConformance {
        properties,
        relationships,
        property_declarations,
        relationship_declarations,
        allows_additional_properties: lineage_boolean(
            graph,
            &lineage,
            "allows_additional_properties",
        )?,
        allows_additional_relationships: lineage_boolean(
            graph,
            &lineage,
            "allows_additional_relationships",
        )?,
    })
}

fn effective_property_declarations(
    graph: &CanonicalDescriptorGraph,
    node: CanonicalNodeId,
) -> Result<Vec<PropertyDeclaration>, ConformanceBuildError> {
    describing_type(graph, &node).map_err(ConformanceBuildError::Graph)?;
    let lineage =
        effective_descriptor_lineage(graph, &node).map_err(ConformanceBuildError::Graph)?;
    let property_nodes = effective_named_members(
        graph,
        &lineage,
        "InstanceProperties",
        "property",
        &["property_name", "type_name"],
    )?;
    let declarations = property_nodes
        .into_iter()
        .map(|descriptor| property_declaration(graph, descriptor))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(declarations)
}

fn effective_named_members(
    graph: &CanonicalDescriptorGraph,
    lineage: &[CanonicalNodeId],
    relationship: &str,
    declaration_kind: &'static str,
    name_properties: &[&str],
) -> Result<Vec<CanonicalNodeId>, ConformanceBuildError> {
    collect_named_members_from_lineage(
        graph,
        lineage.iter().copied(),
        &relationship.to_string(),
        declaration_kind,
        |node| {
            let holon =
                graph.holon(*node).ok_or(CanonicalGraphError::UnknownNode { node: *node })?;
            name_properties
                .iter()
                .find_map(|property| string_property(holon, property))
                .map(ToString::to_string)
                .ok_or_else(|| CanonicalGraphError::InvalidDescriptorData {
                    node: *node,
                    message: format!(
                        "{declaration_kind} descriptor has no semantic name in {}",
                        name_properties.join(" or ")
                    ),
                })
        },
    )
    .map_err(ConformanceBuildError::Graph)
}

fn property_declaration(
    graph: &CanonicalDescriptorGraph,
    descriptor: CanonicalNodeId,
) -> Result<PropertyDeclaration, ConformanceBuildError> {
    let holon = graph
        .holon(descriptor)
        .ok_or_else(|| invalid_descriptor(graph, descriptor, "missing property descriptor"))?;
    let declared_name = string_property(holon, "property_name")
        .or_else(|| string_property(holon, "type_name"))
        .ok_or_else(|| {
            invalid_descriptor(
                graph,
                descriptor,
                "property descriptor has no property_name or type_name",
            )
        })?;
    let required = literal_property(holon, "is_required")
        .and_then(LiteralValue::as_bool)
        .ok_or_else(|| invalid_descriptor(graph, descriptor, "missing Boolean is_required"))?;
    let value_types = graph
        .related_members(&descriptor, &"ValueType".to_string())
        .map_err(|error| ConformanceBuildError::Graph(DescriptorSemanticsError::Access(error)))?;
    let value_policy = match value_types.as_slice() {
        [] => ValuePolicy::Any,
        [value_type] => value_policy(graph, *value_type)?,
        many => {
            return Err(invalid_descriptor(
                graph,
                descriptor,
                &format!("property descriptor has {} ValueType targets; expected one", many.len()),
            ))
        }
    };

    Ok(PropertyDeclaration { name: declared_name.to_string(), required, value_policy })
}

fn align_property_names(properties: &mut [PropertyValue], declarations: &[PropertyDeclaration]) {
    for property in properties {
        let candidates = declarations
            .iter()
            .filter(|declaration| {
                normalized_member_name(&declaration.name) == normalized_member_name(&property.name)
            })
            .collect::<Vec<_>>();
        if let [declaration] = candidates.as_slice() {
            property.name = declaration.name.clone();
        }
    }
}

fn normalized_member_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn relationship_declaration(
    graph: &CanonicalDescriptorGraph,
    descriptor: CanonicalNodeId,
) -> Result<RelationshipDeclaration, ConformanceBuildError> {
    let holon = graph
        .holon(descriptor)
        .ok_or_else(|| invalid_descriptor(graph, descriptor, "missing relationship descriptor"))?;
    let name = string_property(holon, "relationship_name")
        .or_else(|| string_property(holon, "type_name"))
        .ok_or_else(|| {
            invalid_descriptor(
                graph,
                descriptor,
                "relationship descriptor has no relationship_name or type_name",
            )
        })?;
    let minimum = integer_property(holon, "min_cardinality")
        .ok_or_else(|| invalid_descriptor(graph, descriptor, "missing integer min_cardinality"))?;
    let maximum = integer_property(holon, "max_cardinality")
        .ok_or_else(|| invalid_descriptor(graph, descriptor, "missing integer max_cardinality"))?;
    if minimum < 0 || maximum < minimum {
        return Err(invalid_descriptor(
            graph,
            descriptor,
            &format!("invalid relationship cardinality {minimum}..{maximum}"),
        ));
    }
    Ok(RelationshipDeclaration {
        name: name.to_string(),
        minimum: usize::try_from(minimum).unwrap_or(usize::MAX),
        maximum: usize::try_from(maximum).unwrap_or(usize::MAX),
    })
}

fn value_policy(
    graph: &CanonicalDescriptorGraph,
    value_type: CanonicalNodeId,
) -> Result<ValuePolicy, ConformanceBuildError> {
    let holon = graph
        .holon(value_type)
        .ok_or_else(|| invalid_descriptor(graph, value_type, "missing value descriptor"))?;
    let variants = graph
        .related_members(&value_type, &"Variants".to_string())
        .map_err(|error| ConformanceBuildError::Graph(DescriptorSemanticsError::Access(error)))?
        .into_iter()
        .flat_map(|variant| {
            let mut aliases = Vec::new();
            if let Some(key) = graph.key(variant) {
                aliases.push(key.to_string());
            }
            if let Some(type_name) =
                graph.holon(variant).and_then(|holon| string_property(holon, "type_name"))
            {
                aliases.push(type_name.to_string());
            }
            aliases
        })
        .collect();
    Ok(value_policy_for_type_kind(string_property(holon, "TypeKind"), variants))
}

fn lineage_boolean(
    graph: &CanonicalDescriptorGraph,
    lineage: &[CanonicalNodeId],
    property: &str,
) -> Result<bool, ConformanceBuildError> {
    let mut values = Vec::with_capacity(lineage.len());
    for node in lineage {
        let holon = graph.holon(*node).ok_or_else(|| ConformanceBuildError::InvalidDescriptor {
            descriptor: format!("{node:?}"),
            message: "canonical holon is absent".to_string(),
        })?;
        let value =
            literal_property(holon, property).and_then(LiteralValue::as_bool).ok_or_else(|| {
                invalid_descriptor(graph, *node, &format!("missing Boolean {property}"))
            })?;
        values.push(value);
    }
    compose_restrictive_boolean(values).ok_or_else(|| ConformanceBuildError::InvalidDescriptor {
        descriptor: "<empty describing type lineage>".to_string(),
        message: format!("cannot resolve {property}"),
    })
}

fn string_property<'a>(holon: &'a crate::CanonicalHolon, name: &str) -> Option<&'a str> {
    literal_property(holon, name).and_then(LiteralValue::as_str)
}

fn integer_property(holon: &crate::CanonicalHolon, name: &str) -> Option<i64> {
    literal_property(holon, name).and_then(LiteralValue::as_i64)
}

fn literal_property<'a>(holon: &'a crate::CanonicalHolon, name: &str) -> Option<&'a LiteralValue> {
    let normalized = normalized_member_name(name);
    holon
        .properties
        .iter()
        .find(|(candidate, _)| normalized_member_name(candidate) == normalized)
        .map(|(_, value)| value)
}

fn conformance_value(value: &LiteralValue) -> ConformanceValue {
    match value {
        LiteralValue::Null => ConformanceValue::Null,
        LiteralValue::Boolean(value) => ConformanceValue::Boolean(*value),
        LiteralValue::Integer(value) => ConformanceValue::Integer(*value),
        LiteralValue::Number(value) => ConformanceValue::Number(value.clone()),
        LiteralValue::String(value) => ConformanceValue::String(value.clone()),
        LiteralValue::Array(_) => ConformanceValue::Array,
        LiteralValue::Object(_) => ConformanceValue::Object,
    }
}

fn invalid_descriptor(
    graph: &CanonicalDescriptorGraph,
    descriptor: CanonicalNodeId,
    message: &str,
) -> ConformanceBuildError {
    ConformanceBuildError::InvalidDescriptor {
        descriptor: graph.key(descriptor).unwrap_or("<unknown>").to_string(),
        message: message.to_string(),
    }
}

fn project_violation(
    holon: &str,
    origin: Option<crate::Origin>,
    violation: ConformanceViolation,
) -> Diagnostic {
    let kind = match violation {
        ConformanceViolation::MissingRequiredProperty { property } => {
            DiagnosticKind::MissingConformanceProperty { holon: holon.to_string(), property }
        }
        ConformanceViolation::AdditionalProperty { property } => {
            DiagnosticKind::AdditionalConformanceProperty { holon: holon.to_string(), property }
        }
        ConformanceViolation::WrongValueKind { property, expected } => {
            DiagnosticKind::InvalidConformanceValue {
                holon: holon.to_string(),
                property,
                value: "wrong literal kind".to_string(),
                expected: expected.to_string(),
            }
        }
        ConformanceViolation::UndeclaredEnumVariant { property, variant } => {
            DiagnosticKind::InvalidConformanceValue {
                holon: holon.to_string(),
                property,
                value: variant,
                expected: "a variant declared by the resolved enum ValueType".to_string(),
            }
        }
        ConformanceViolation::AdditionalRelationship { relationship } => {
            DiagnosticKind::AdditionalConformanceRelationship {
                holon: holon.to_string(),
                relationship,
            }
        }
        ConformanceViolation::RelationshipCardinality {
            relationship,
            actual,
            minimum,
            maximum,
        } => DiagnosticKind::ConformanceRelationshipCardinality {
            holon: holon.to_string(),
            relationship,
            actual,
            minimum,
            maximum,
        },
    };
    Diagnostic::error(DiagnosticLayer::SchemaAware, kind, origin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DescriptorKind, LiteralRelationship, Origin, ReferenceRole, SemanticModel,
        SemanticReference, SourceKind, SymbolIndex, TypeDescriptor,
    };

    fn descriptor_of_kind(key: &str, kind: DescriptorKind) -> TypeDescriptor {
        TypeDescriptor::new(key, key, kind, "TestSchema", Origin::new(SourceKind::Generated))
    }

    fn descriptor(key: &str) -> TypeDescriptor {
        descriptor_of_kind(key, DescriptorKind::HolonType)
    }

    fn string_property_value(descriptor: &mut TypeDescriptor, name: &str, value: &str) {
        descriptor.literal_properties.insert(name, LiteralValue::String(value.to_string()));
    }

    #[test]
    fn invalid_enum_value_is_rejected_through_descriptor_data() {
        let mut model = SemanticModel::new();
        let mut type_descriptor = descriptor("TypeDescriptor.HolonType");
        type_descriptor.described_by.push(SemanticReference::unresolved(
            ReferenceRole::DescribedBy,
            "TypeDescriptor.HolonType",
        ));
        model.push_descriptor(type_descriptor);

        let mut enum_type = descriptor_of_kind("KindEnum", DescriptorKind::Enum);
        enum_type.instance_type_kind = Some("TypeKind.Value.Enum".to_string());
        enum_type.literal_relationships.push(LiteralRelationship {
            name: "Variants".to_string(),
            targets: vec!["Kind.One".to_string(), "Kind.Two".to_string()],
        });
        model.push_descriptor(enum_type);
        model.push_descriptor(descriptor_of_kind("Kind.One", DescriptorKind::EnumVariant));
        model.push_descriptor(descriptor_of_kind("Kind.Two", DescriptorKind::EnumVariant));

        let mut property = descriptor_of_kind("Kind.PropertyType", DescriptorKind::PropertyType);
        string_property_value(&mut property, "property_name", "kind");
        property.property_required = Some(true);
        property.literal_relationships.push(LiteralRelationship {
            name: "ValueType".to_string(),
            targets: vec!["KindEnum".to_string()],
        });
        model.push_descriptor(property);

        let mut thing_type = descriptor("Thing.HolonType");
        thing_type.described_by.push(SemanticReference::unresolved(
            ReferenceRole::DescribedBy,
            "TypeDescriptor.HolonType",
        ));
        thing_type.instance_properties.push(SemanticReference::unresolved(
            ReferenceRole::InstanceProperty,
            "Kind.PropertyType",
        ));
        thing_type
            .literal_properties
            .insert("allows_additional_properties".to_string(), LiteralValue::Boolean(false));
        thing_type
            .literal_properties
            .insert("allows_additional_relationships".to_string(), LiteralValue::Boolean(false));
        model.push_descriptor(thing_type);

        let mut thing = descriptor("Thing.One");
        thing
            .described_by
            .push(SemanticReference::unresolved(ReferenceRole::DescribedBy, "Thing.HolonType"));
        string_property_value(&mut thing, "kind", "Kind.Unknown");
        model.push_descriptor(thing);

        let (symbols, index_diagnostics) = SymbolIndex::build(&mut model);
        assert!(index_diagnostics.is_empty(), "{index_diagnostics:?}");
        let graph = CanonicalDescriptorGraph::new(&model, &symbols).expect("graph");
        let diagnostics = validate_canonical_holon_conformance(
            &graph,
            graph.node_by_key("Thing.One").expect("thing"),
        );

        assert!(
            diagnostics.iter().any(|diagnostic| matches!(
                &diagnostic.kind,
                DiagnosticKind::InvalidConformanceValue { property, value, .. }
                    if property == "kind" && value == "Kind.Unknown"
            )),
            "{diagnostics:#?}"
        );
    }
}
