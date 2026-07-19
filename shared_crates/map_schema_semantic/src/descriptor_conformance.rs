//! Descriptor-driven conformance over Canonical Holon IR.
//!
//! This module is the representation adapter: it resolves effective declarations from canonical
//! holons, delegates policy evaluation to `descriptor_semantics`, and projects neutral violations
//! into shared diagnostics. It intentionally contains no property-specific validation rules.

use std::collections::HashSet;

use descriptor_semantics::{
    effective_descriptor_lineage, property_requirement, validate_holon_conformance,
    value_policy_for_type_kind, ConformanceValue, ConformanceViolation, DescriptorGraph,
    DescriptorSemanticsError, HolonConformance, PropertyDeclaration, PropertyValue,
    RelationshipDeclaration, RelationshipValue, ValuePolicy,
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
    graph.nodes().flat_map(|node| validate_canonical_holon_conformance(graph, node)).collect()
}

/// Validates one canonical holon using declarations resolved from its effective descriptor graph.
pub fn validate_canonical_holon_conformance(
    graph: &CanonicalDescriptorGraph,
    node: CanonicalNodeId,
) -> Vec<Diagnostic> {
    let input = match conformance_input(graph, node) {
        Ok(input) => input,
        Err(ConformanceBuildError::Graph(error)) => return vec![graph.diagnostic(error)],
        Err(ConformanceBuildError::InvalidDescriptor { descriptor, message }) => {
            return vec![Diagnostic::error(
                DiagnosticLayer::SchemaAware,
                DiagnosticKind::DescriptorGraphAccess {
                    holon: graph.key(node).unwrap_or("<unknown>").to_string(),
                    relationship: "descriptor conformance".to_string(),
                    target: Some(descriptor),
                    message,
                },
                graph.holon(node).map(|holon| holon.origin.clone()),
            )]
        }
    };

    let holon = graph.key(node).unwrap_or("<unknown>").to_string();
    let origin = graph.holon(node).map(|holon| holon.origin.clone());
    validate_holon_conformance(&input)
        .into_iter()
        .map(|violation| project_violation(&holon, origin.clone(), violation))
        .collect()
}

#[derive(Debug)]
enum ConformanceBuildError {
    Graph(DescriptorSemanticsError<CanonicalGraphError, CanonicalNodeId>),
    InvalidDescriptor { descriptor: String, message: String },
}

fn conformance_input(
    graph: &CanonicalDescriptorGraph,
    node: CanonicalNodeId,
) -> Result<HolonConformance, ConformanceBuildError> {
    let holon = graph.holon(node).ok_or_else(|| ConformanceBuildError::InvalidDescriptor {
        descriptor: format!("{node:?}"),
        message: "canonical holon is absent".to_string(),
    })?;
    let lineage =
        effective_descriptor_lineage(graph, &node).map_err(ConformanceBuildError::Graph)?;
    let property_nodes = effective_members(graph, &lineage, "InstanceProperties")?;
    let relationship_nodes = effective_members(graph, &lineage, "InstanceRelationships")?;

    let property_declarations = property_nodes
        .into_iter()
        .map(|descriptor| property_declaration(graph, descriptor))
        .collect::<Result<Vec<_>, _>>()?;
    let relationship_declarations = relationship_nodes
        .into_iter()
        .map(|descriptor| relationship_declaration(graph, descriptor))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(HolonConformance {
        properties: holon
            .properties
            .iter()
            .map(|(name, value)| PropertyValue {
                name: name.clone(),
                value: conformance_value(value),
            })
            .collect(),
        relationships: holon
            .relationships
            .iter()
            .map(|relationship| RelationshipValue {
                name: relationship.name.clone(),
                target_count: relationship.targets.len(),
            })
            .collect(),
        property_declarations,
        relationship_declarations,
        allows_additional_properties: lineage_boolean(
            graph,
            &lineage,
            "allows_additional_properties",
        ),
        allows_additional_relationships: lineage_boolean(
            graph,
            &lineage,
            "allows_additional_relationships",
        ),
    })
}

fn effective_members(
    graph: &CanonicalDescriptorGraph,
    lineage: &[CanonicalNodeId],
    relationship: &str,
) -> Result<Vec<CanonicalNodeId>, ConformanceBuildError> {
    let mut members = Vec::new();
    let mut seen = HashSet::new();
    for descriptor in lineage {
        for member in
            graph.related_members(descriptor, &relationship.to_string()).map_err(|error| {
                ConformanceBuildError::Graph(DescriptorSemanticsError::Access(error))
            })?
        {
            if seen.insert(graph.identity(&member)) {
                members.push(member);
            }
        }
    }
    Ok(members)
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
    let requirement = property_requirement(declared_name);
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

    Ok(PropertyDeclaration {
        name: requirement.name.to_string(),
        required: requirement.required,
        value_policy,
    })
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
    let minimum = integer_property(holon, "min_cardinality").unwrap_or(0);
    let maximum = integer_property(holon, "max_cardinality").unwrap_or(i64::MAX);
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
        .filter_map(|variant| graph.key(variant).map(ToString::to_string))
        .collect();
    Ok(value_policy_for_type_kind(string_property(holon, "instance_type_kind"), variants))
}

fn lineage_boolean(
    graph: &CanonicalDescriptorGraph,
    lineage: &[CanonicalNodeId],
    property: &str,
) -> bool {
    lineage
        .iter()
        .find_map(|node| graph.holon(*node)?.properties.get(property)?.as_bool())
        .unwrap_or(false)
}

fn string_property<'a>(holon: &'a crate::CanonicalHolon, name: &str) -> Option<&'a str> {
    holon.properties.get(name).and_then(LiteralValue::as_str)
}

fn integer_property(holon: &crate::CanonicalHolon, name: &str) -> Option<i64> {
    holon.properties.get(name).and_then(LiteralValue::as_i64)
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
        model.push_descriptor(descriptor("TypeDescriptor.HolonType"));

        let mut enum_type = descriptor_of_kind("KindEnum", DescriptorKind::Enum);
        string_property_value(&mut enum_type, "instance_type_kind", "TypeKind.Value.Enum");
        enum_type.literal_relationships.push(LiteralRelationship {
            name: "Variants".to_string(),
            targets: vec!["Kind.One".to_string(), "Kind.Two".to_string()],
        });
        model.push_descriptor(enum_type);
        model.push_descriptor(descriptor_of_kind("Kind.One", DescriptorKind::EnumVariant));
        model.push_descriptor(descriptor_of_kind("Kind.Two", DescriptorKind::EnumVariant));

        let mut property = descriptor_of_kind("Kind.PropertyType", DescriptorKind::PropertyType);
        string_property_value(&mut property, "property_name", "kind");
        property.literal_relationships.push(LiteralRelationship {
            name: "ValueType".to_string(),
            targets: vec!["KindEnum".to_string()],
        });
        model.push_descriptor(property);

        let mut thing_type = descriptor("Thing.HolonType");
        thing_type.instance_properties.push(SemanticReference::unresolved(
            ReferenceRole::InstanceProperty,
            "Kind.PropertyType",
        ));
        thing_type.allows_additional_properties = false;
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

        assert!(matches!(
            diagnostics.as_slice(),
            [Diagnostic {
                kind: DiagnosticKind::InvalidConformanceValue { property, value, .. },
                ..
            }] if property == "kind" && value == "Kind.Unknown"
        ));
    }
}
