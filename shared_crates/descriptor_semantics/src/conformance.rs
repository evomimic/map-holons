use std::collections::HashSet;

/// Composes an inherited openness policy restrictively.
///
/// A `false` value anywhere in the type lineage closes the corresponding surface. `None` means
/// no descriptor in the supplied lineage declared the required policy value.
pub fn compose_restrictive_boolean(values: impl IntoIterator<Item = bool>) -> Option<bool> {
    let mut values = values.into_iter();
    let first = values.next()?;
    Some(values.fold(first, |effective, value| effective && value))
}

/// Source-neutral literal shape consumed by descriptor conformance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConformanceValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Number(String),
    String(String),
    Array,
    Object,
}

/// Value policy resolved from a property's `ValueType` descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValuePolicy {
    Any,
    Boolean,
    Integer,
    String,
    Enum { variants: Vec<String> },
}

/// Resolves primitive/enum policy from the descriptor-authored TypeKind identity.
///
/// Unknown and structural value kinds remain unconstrained here; adapters may still validate them
/// through relationships or constraint descriptors without inventing a primitive classification.
pub fn value_policy_for_type_kind(
    type_kind: Option<&str>,
    enum_variants: Vec<String>,
) -> ValuePolicy {
    match type_kind {
        Some("TypeKind.Value.Boolean") => ValuePolicy::Boolean,
        Some("TypeKind.Value.Integer") => ValuePolicy::Integer,
        Some("TypeKind.Value.String") => ValuePolicy::String,
        Some("TypeKind.Value.Enum") => ValuePolicy::Enum { variants: enum_variants },
        _ => ValuePolicy::Any,
    }
}

/// Effective property declaration supplied by a descriptor graph adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyDeclaration {
    pub name: String,
    pub required: bool,
    pub value_policy: ValuePolicy,
}

/// Effective relationship declaration supplied by a descriptor graph adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipDeclaration {
    pub name: String,
    pub minimum: usize,
    pub maximum: usize,
}

/// One authored property on the holon being validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyValue {
    pub name: String,
    pub value: ConformanceValue,
}

/// One authored relationship collection on the holon being validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipValue {
    pub name: String,
    pub target_count: usize,
}

/// Fully resolved, representation-neutral input to the conformance evaluator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HolonConformance {
    pub properties: Vec<PropertyValue>,
    pub relationships: Vec<RelationshipValue>,
    pub property_declarations: Vec<PropertyDeclaration>,
    pub relationship_declarations: Vec<RelationshipDeclaration>,
    pub allows_additional_properties: bool,
    pub allows_additional_relationships: bool,
}

/// Descriptor-conformance failure independent of runtime or authoring representations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConformanceViolation {
    MissingRequiredProperty { property: String },
    AdditionalProperty { property: String },
    WrongValueKind { property: String, expected: &'static str },
    UndeclaredEnumVariant { property: String, variant: String },
    AdditionalRelationship { relationship: String },
    RelationshipCardinality { relationship: String, actual: usize, minimum: usize, maximum: usize },
}

/// Evaluates one holon against already-resolved effective descriptor declarations.
///
/// Graph traversal and descriptor projection happen before this function. Keeping the evaluator
/// over plain data makes the same policy usable by Canonical IR and bound runtime adapters.
pub fn validate_holon_conformance(input: &HolonConformance) -> Vec<ConformanceViolation> {
    let mut violations = Vec::new();
    let property_names =
        input.properties.iter().map(|property| property.name.as_str()).collect::<HashSet<_>>();

    for declaration in &input.property_declarations {
        if declaration.required && !property_names.contains(declaration.name.as_str()) {
            violations.push(ConformanceViolation::MissingRequiredProperty {
                property: declaration.name.clone(),
            });
        }
    }

    for property in &input.properties {
        let Some(declaration) = input
            .property_declarations
            .iter()
            .find(|declaration| declaration.name == property.name)
        else {
            if !input.allows_additional_properties {
                violations.push(ConformanceViolation::AdditionalProperty {
                    property: property.name.clone(),
                });
            }
            continue;
        };
        validate_value(property, &declaration.value_policy, &mut violations);
    }

    let mut relationship_totals: Vec<(&str, usize)> = Vec::new();
    for relationship in &input.relationships {
        if let Some((_, total)) =
            relationship_totals.iter_mut().find(|(name, _)| *name == relationship.name)
        {
            *total = total.saturating_add(relationship.target_count);
        } else {
            relationship_totals.push((&relationship.name, relationship.target_count));
        }
    }

    for (relationship_name, target_count) in relationship_totals {
        let Some(declaration) = input
            .relationship_declarations
            .iter()
            .find(|declaration| declaration.name == relationship_name)
        else {
            if !input.allows_additional_relationships {
                violations.push(ConformanceViolation::AdditionalRelationship {
                    relationship: relationship_name.to_string(),
                });
            }
            continue;
        };
        if let Err(error) =
            validate_cardinality(target_count, declaration.minimum, declaration.maximum)
        {
            violations.push(ConformanceViolation::RelationshipCardinality {
                relationship: relationship_name.to_string(),
                actual: error.actual,
                minimum: error.minimum,
                maximum: error.maximum,
            });
        }
    }

    for declaration in &input.relationship_declarations {
        if input.relationships.iter().any(|relationship| relationship.name == declaration.name) {
            continue;
        }
        if declaration.minimum > 0 {
            violations.push(ConformanceViolation::RelationshipCardinality {
                relationship: declaration.name.clone(),
                actual: 0,
                minimum: declaration.minimum,
                maximum: declaration.maximum,
            });
        }
    }

    violations
}

fn validate_value(
    property: &PropertyValue,
    policy: &ValuePolicy,
    violations: &mut Vec<ConformanceViolation>,
) {
    let expected = match (policy, &property.value) {
        (ValuePolicy::Any, _) => return,
        (ValuePolicy::Boolean, ConformanceValue::Boolean(_)) => return,
        (ValuePolicy::Integer, ConformanceValue::Integer(_)) => return,
        (ValuePolicy::String, ConformanceValue::String(_)) => return,
        (ValuePolicy::Enum { variants }, ConformanceValue::String(variant)) => {
            if validate_enum_variant(variant, variants.iter().map(String::as_str)).is_err() {
                violations.push(ConformanceViolation::UndeclaredEnumVariant {
                    property: property.name.clone(),
                    variant: variant.clone(),
                });
            }
            return;
        }
        (ValuePolicy::Boolean, _) => "boolean",
        (ValuePolicy::Integer, _) => "integer",
        (ValuePolicy::String, _) => "string",
        (ValuePolicy::Enum { .. }, _) => "enum variant string",
    };
    violations
        .push(ConformanceViolation::WrongValueKind { property: property.name.clone(), expected });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardinalityViolation {
    pub actual: usize,
    pub minimum: usize,
    pub maximum: usize,
}

pub fn validate_cardinality(
    actual: usize,
    minimum: usize,
    maximum: usize,
) -> Result<(), CardinalityViolation> {
    if actual >= minimum && actual <= maximum {
        Ok(())
    } else {
        Err(CardinalityViolation { actual, minimum, maximum })
    }
}

use crate::validate_enum_variant;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openness_composes_restrictively() {
        assert_eq!(compose_restrictive_boolean([true, true]), Some(true));
        assert_eq!(compose_restrictive_boolean([true, false, true]), Some(false));
        assert_eq!(compose_restrictive_boolean([]), None);
    }

    #[test]
    fn cardinality_is_inclusive() {
        assert!(validate_cardinality(1, 1, 2).is_ok());
        assert!(validate_cardinality(2, 1, 2).is_ok());
        assert_eq!(
            validate_cardinality(3, 1, 2),
            Err(CardinalityViolation { actual: 3, minimum: 1, maximum: 2 })
        );
    }

    #[test]
    fn evaluates_descriptor_resolved_shape_without_property_specific_rules() {
        let input = HolonConformance {
            properties: vec![
                PropertyValue {
                    name: "status".to_string(),
                    value: ConformanceValue::String("Status.Unknown".to_string()),
                },
                PropertyValue { name: "extra".to_string(), value: ConformanceValue::Boolean(true) },
            ],
            relationships: vec![RelationshipValue { name: "Owner".to_string(), target_count: 2 }],
            property_declarations: vec![
                PropertyDeclaration {
                    name: "name".to_string(),
                    required: true,
                    value_policy: ValuePolicy::String,
                },
                PropertyDeclaration {
                    name: "status".to_string(),
                    required: false,
                    value_policy: ValuePolicy::Enum {
                        variants: vec!["Status.Open".to_string(), "Status.Closed".to_string()],
                    },
                },
            ],
            relationship_declarations: vec![RelationshipDeclaration {
                name: "Owner".to_string(),
                minimum: 1,
                maximum: 1,
            }],
            allows_additional_properties: false,
            allows_additional_relationships: false,
        };

        assert_eq!(
            validate_holon_conformance(&input),
            vec![
                ConformanceViolation::MissingRequiredProperty { property: "name".to_string() },
                ConformanceViolation::UndeclaredEnumVariant {
                    property: "status".to_string(),
                    variant: "Status.Unknown".to_string(),
                },
                ConformanceViolation::AdditionalProperty { property: "extra".to_string() },
                ConformanceViolation::RelationshipCardinality {
                    relationship: "Owner".to_string(),
                    actual: 2,
                    minimum: 1,
                    maximum: 1,
                },
            ]
        );
    }

    #[test]
    fn value_policy_is_derived_from_descriptor_type_kind() {
        assert_eq!(
            value_policy_for_type_kind(
                Some("TypeKind.Value.Enum"),
                vec!["Status.Open".to_string()]
            ),
            ValuePolicy::Enum { variants: vec!["Status.Open".to_string()] }
        );
        assert_eq!(value_policy_for_type_kind(Some("Custom.ValueKind"), vec![]), ValuePolicy::Any);
    }

    #[test]
    fn repeated_relationship_entries_share_one_cardinality_collection() {
        let input = HolonConformance {
            properties: vec![],
            relationships: vec![
                RelationshipValue { name: "Owner".to_string(), target_count: 1 },
                RelationshipValue { name: "Owner".to_string(), target_count: 1 },
            ],
            property_declarations: vec![],
            relationship_declarations: vec![RelationshipDeclaration {
                name: "Owner".to_string(),
                minimum: 0,
                maximum: 1,
            }],
            allows_additional_properties: false,
            allows_additional_relationships: false,
        };

        assert_eq!(
            validate_holon_conformance(&input),
            vec![ConformanceViolation::RelationshipCardinality {
                relationship: "Owner".to_string(),
                actual: 2,
                minimum: 0,
                maximum: 1,
            }]
        );
    }
}
