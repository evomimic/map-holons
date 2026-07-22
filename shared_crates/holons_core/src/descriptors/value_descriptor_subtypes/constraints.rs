use crate::descriptors::accessor_helpers::{descriptor_label, require_bool, require_integer};
use crate::descriptors::inheritance::{flatten_related_members, walk_extends_chain};
use crate::descriptors::TypeHeader;
use crate::reference_layer::HolonReference;
use core_types::{HolonError, SchemaInvalidityKind};
use descriptor_semantics::{
    validate_integer_maximum, validate_integer_minimum, validate_string_maximum_length,
    validate_string_minimum_length,
};
use type_names::{CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct MinimumValueConstraint {
    pub value: i64,
    pub inclusive: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct MaximumValueConstraint {
    pub value: i64,
    pub inclusive: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct MinimumLengthConstraint {
    pub length: i64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct MaximumLengthConstraint {
    pub length: i64,
}

pub(crate) trait IntegerConstraintValidation {
    /// Validates one concrete integer value against this resolved constraint.
    fn is_valid(&self, value: i64, descriptor_label: &str) -> Result<(), HolonError>;
}

pub(crate) trait StringConstraintValidation {
    /// Validates one concrete string value against this resolved constraint.
    fn is_valid(&self, value: &str, descriptor_label: &str) -> Result<(), HolonError>;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum IntegerConstraint {
    Minimum(MinimumValueConstraint),
    Maximum(MaximumValueConstraint),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum StringConstraint {
    MinimumLength(MinimumLengthConstraint),
    MaximumLength(MaximumLengthConstraint),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ConstraintFamily {
    Integer,
    String,
    Bytes,
    ValueArray,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ConstraintKind {
    MinimumValue,
    MaximumValue,
    MinimumLength,
    MaximumLength,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ConstraintClassification {
    family: Option<ConstraintFamily>,
    kind: Option<ConstraintKind>,
}

impl IntegerConstraintValidation for MinimumValueConstraint {
    fn is_valid(&self, value: i64, descriptor_label: &str) -> Result<(), HolonError> {
        if validate_integer_minimum(value, self.value, self.inclusive).is_ok() {
            return Ok(());
        }

        Err(HolonError::IntegerOutOfRange {
            value,
            min: Some(self.value),
            max: None,
            min_inclusive: self.inclusive,
            max_inclusive: true,
            descriptor: descriptor_label.to_string(),
        })
    }
}

impl IntegerConstraintValidation for MaximumValueConstraint {
    fn is_valid(&self, value: i64, descriptor_label: &str) -> Result<(), HolonError> {
        if validate_integer_maximum(value, self.value, self.inclusive).is_ok() {
            return Ok(());
        }

        Err(HolonError::IntegerOutOfRange {
            value,
            min: None,
            max: Some(self.value),
            min_inclusive: true,
            max_inclusive: self.inclusive,
            descriptor: descriptor_label.to_string(),
        })
    }
}

impl StringConstraintValidation for MinimumLengthConstraint {
    fn is_valid(&self, value: &str, descriptor_label: &str) -> Result<(), HolonError> {
        let length = value.chars().count();
        if validate_string_minimum_length(value, self.length).is_ok() {
            return Ok(());
        }

        Err(HolonError::StringLengthOutOfRange {
            length,
            min: Some(self.length),
            max: None,
            descriptor: descriptor_label.to_string(),
        })
    }
}

impl StringConstraintValidation for MaximumLengthConstraint {
    fn is_valid(&self, value: &str, descriptor_label: &str) -> Result<(), HolonError> {
        let length = value.chars().count();
        if validate_string_maximum_length(value, self.length).is_ok() {
            return Ok(());
        }

        Err(HolonError::StringLengthOutOfRange {
            length,
            min: None,
            max: Some(self.length),
            descriptor: descriptor_label.to_string(),
        })
    }
}

impl IntegerConstraintValidation for IntegerConstraint {
    fn is_valid(&self, value: i64, descriptor_label: &str) -> Result<(), HolonError> {
        match self {
            Self::Minimum(constraint) => constraint.is_valid(value, descriptor_label),
            Self::Maximum(constraint) => constraint.is_valid(value, descriptor_label),
        }
    }
}

impl StringConstraintValidation for StringConstraint {
    fn is_valid(&self, value: &str, descriptor_label: &str) -> Result<(), HolonError> {
        match self {
            Self::MinimumLength(constraint) => constraint.is_valid(value, descriptor_label),
            Self::MaximumLength(constraint) => constraint.is_valid(value, descriptor_label),
        }
    }
}

/// Resolves inherited `Constraints` relationships into executable integer constraints.
pub(crate) fn resolve_integer_constraints(
    value_type: &HolonReference,
) -> Result<Vec<IntegerConstraint>, HolonError> {
    let mut resolved_constraints = Vec::new();

    // Constraint discovery: value descriptors inherit constraint relationships
    // through the same flattened Extends traversal used by other descriptors.
    for constraint_holon in
        flatten_related_members(value_type, CoreRelationshipTypeName::Constraints)?
    {
        let classification = classify_constraint(&constraint_holon)?;
        require_family(
            value_type,
            &constraint_holon,
            classification.family,
            ConstraintFamily::Integer,
        )?;

        let constraint = match classification.kind {
            Some(ConstraintKind::MinimumValue) => {
                IntegerConstraint::Minimum(MinimumValueConstraint {
                    value: require_constraint_integer_value(value_type, &constraint_holon)?,
                    inclusive: require_constraint_is_inclusive(value_type, &constraint_holon)?,
                })
            }
            Some(ConstraintKind::MaximumValue) => {
                IntegerConstraint::Maximum(MaximumValueConstraint {
                    value: require_constraint_integer_value(value_type, &constraint_holon)?,
                    inclusive: require_constraint_is_inclusive(value_type, &constraint_holon)?,
                })
            }
            // Family matched Integer but the concrete kind is either
            // unclassified (no recognized kind anchor in the Extends chain)
            // or belongs to a different family's kind set. Both cases are
            // unsupported on the integer executable path.
            _ => return Err(unsupported_constraint(value_type, &constraint_holon)),
        };

        resolved_constraints.push(constraint);
    }

    validate_integer_coherence(value_type, &resolved_constraints)?;
    Ok(resolved_constraints)
}

/// Resolves inherited `Constraints` relationships into executable string constraints.
pub(crate) fn resolve_string_constraints(
    value_type: &HolonReference,
) -> Result<Vec<StringConstraint>, HolonError> {
    let mut resolved_constraints = Vec::new();

    // Constraint discovery: local and inherited constraint holons are resolved
    // into a family-specific runtime shape before validation executes.
    for constraint_holon in
        flatten_related_members(value_type, CoreRelationshipTypeName::Constraints)?
    {
        let classification = classify_constraint(&constraint_holon)?;
        require_family(
            value_type,
            &constraint_holon,
            classification.family,
            ConstraintFamily::String,
        )?;

        let constraint = match classification.kind {
            Some(ConstraintKind::MinimumLength) => {
                StringConstraint::MinimumLength(MinimumLengthConstraint {
                    length: require_constraint_length(value_type, &constraint_holon)?,
                })
            }
            Some(ConstraintKind::MaximumLength) => {
                StringConstraint::MaximumLength(MaximumLengthConstraint {
                    length: require_constraint_length(value_type, &constraint_holon)?,
                })
            }
            // Family matched String but the concrete kind is either
            // unclassified (no recognized kind anchor in the Extends chain)
            // or belongs to a different family's kind set. Both cases are
            // unsupported on the string executable path.
            _ => return Err(unsupported_constraint(value_type, &constraint_holon)),
        };

        resolved_constraints.push(constraint);
    }

    validate_string_coherence(value_type, &resolved_constraints)?;
    Ok(resolved_constraints)
}

fn classify_constraint(
    constraint_holon: &HolonReference,
) -> Result<ConstraintClassification, HolonError> {
    let mut family = None;
    let mut kind = None;

    // Constraint classification: walk self-first so concrete constraint type
    // names and abstract family anchors can both be discovered from the graph.
    for ancestor in walk_extends_chain(constraint_holon) {
        let ancestor = ancestor?;
        let type_name = TypeHeader::new(&ancestor).type_name()?;
        family = family.or_else(|| family_from_type_name(type_name.0.as_str()));
        kind = kind.or_else(|| kind_from_type_name(type_name.0.as_str()));
        // Both classifiers are first-write-wins (via `or_else`), so once each
        // has been filled there is nothing further up the Extends chain that
        // could change the result. Stop early to avoid touching ancestors
        // we have no decision to make about.
        if family.is_some() && kind.is_some() {
            break;
        }
    }

    Ok(ConstraintClassification { family, kind })
}

fn family_from_type_name(type_name: &str) -> Option<ConstraintFamily> {
    if type_name == CoreHolonTypeName::IntegerValueConstraint.as_holon_name().0.as_str() {
        Some(ConstraintFamily::Integer)
    } else if type_name == CoreHolonTypeName::StringValueConstraint.as_holon_name().0.as_str() {
        Some(ConstraintFamily::String)
    } else if type_name == CoreHolonTypeName::BytesValueConstraint.as_holon_name().0.as_str() {
        Some(ConstraintFamily::Bytes)
    } else if type_name == CoreHolonTypeName::ValueArrayConstraint.as_holon_name().0.as_str() {
        Some(ConstraintFamily::ValueArray)
    } else {
        None
    }
}

fn kind_from_type_name(type_name: &str) -> Option<ConstraintKind> {
    if type_name == CoreHolonTypeName::MinimumValue.as_holon_name().0.as_str() {
        Some(ConstraintKind::MinimumValue)
    } else if type_name == CoreHolonTypeName::MaximumValue.as_holon_name().0.as_str() {
        Some(ConstraintKind::MaximumValue)
    } else if type_name == CoreHolonTypeName::MinimumLength.as_holon_name().0.as_str() {
        Some(ConstraintKind::MinimumLength)
    } else if type_name == CoreHolonTypeName::MaximumLength.as_holon_name().0.as_str() {
        Some(ConstraintKind::MaximumLength)
    } else {
        None
    }
}

fn require_family(
    value_type: &HolonReference,
    constraint_holon: &HolonReference,
    actual_family: Option<ConstraintFamily>,
    expected_family: ConstraintFamily,
) -> Result<(), HolonError> {
    match actual_family {
        Some(family) if family == expected_family => Ok(()),
        Some(family) => Err(schema_invalid(
            value_type,
            SchemaInvalidityKind::IncompatibleConstraintFamily,
            format!(
                "Constraint {} belongs to the {:?} family, but {:?} constraints are required",
                descriptor_label(constraint_holon),
                family,
                expected_family
            ),
        )),
        None => Err(schema_invalid(
            value_type,
            SchemaInvalidityKind::UnclassifiedConstraint,
            format!(
                "Constraint {} does not extend a recognized value constraint family",
                descriptor_label(constraint_holon)
            ),
        )),
    }
}

fn require_constraint_integer_value(
    value_type: &HolonReference,
    constraint_holon: &HolonReference,
) -> Result<i64, HolonError> {
    require_integer(constraint_holon, CorePropertyTypeName::ConstraintIntegerValue)
        .map_err(|error| map_constraint_parameter_error(value_type, constraint_holon, error))
}

fn require_constraint_is_inclusive(
    value_type: &HolonReference,
    constraint_holon: &HolonReference,
) -> Result<bool, HolonError> {
    require_bool(constraint_holon, CorePropertyTypeName::ConstraintIsInclusive)
        .map_err(|error| map_constraint_parameter_error(value_type, constraint_holon, error))
}

fn require_constraint_length(
    value_type: &HolonReference,
    constraint_holon: &HolonReference,
) -> Result<i64, HolonError> {
    require_integer(constraint_holon, CorePropertyTypeName::ConstraintLength)
        .map_err(|error| map_constraint_parameter_error(value_type, constraint_holon, error))
}

fn map_constraint_parameter_error(
    value_type: &HolonReference,
    constraint_holon: &HolonReference,
    error: HolonError,
) -> HolonError {
    match error {
        HolonError::EmptyField(field) => schema_invalid(
            value_type,
            SchemaInvalidityKind::MissingConstraintParameter,
            format!(
                "Constraint {} is missing required property {field}",
                descriptor_label(constraint_holon)
            ),
        ),
        other => other,
    }
}

fn validate_integer_coherence(
    value_type: &HolonReference,
    constraints: &[IntegerConstraint],
) -> Result<(), HolonError> {
    let strongest_minimum = constraints
        .iter()
        .filter_map(|constraint| match constraint {
            IntegerConstraint::Minimum(minimum) => Some(effective_integer_minimum(minimum)),
            IntegerConstraint::Maximum(_) => None,
        })
        .max();
    let strongest_maximum = constraints
        .iter()
        .filter_map(|constraint| match constraint {
            IntegerConstraint::Maximum(maximum) => Some(effective_integer_maximum(maximum)),
            IntegerConstraint::Minimum(_) => None,
        })
        .min();

    if let (Some(minimum), Some(maximum)) = (strongest_minimum, strongest_maximum) {
        if minimum > maximum {
            return Err(schema_invalid(
                value_type,
                SchemaInvalidityKind::ContradictoryConstraints,
                format!(
                    "Effective integer interval is empty: minimum allowed value {minimum}, maximum allowed value {maximum}"
                ),
            ));
        }
    }

    Ok(())
}

fn effective_integer_minimum(constraint: &MinimumValueConstraint) -> i128 {
    i128::from(constraint.value) + if constraint.inclusive { 0 } else { 1 }
}

fn effective_integer_maximum(constraint: &MaximumValueConstraint) -> i128 {
    i128::from(constraint.value) - if constraint.inclusive { 0 } else { 1 }
}

fn validate_string_coherence(
    value_type: &HolonReference,
    constraints: &[StringConstraint],
) -> Result<(), HolonError> {
    let strongest_minimum = constraints
        .iter()
        .filter_map(|constraint| match constraint {
            StringConstraint::MinimumLength(minimum) => Some(minimum.length),
            StringConstraint::MaximumLength(_) => None,
        })
        .max();
    let strongest_maximum = constraints
        .iter()
        .filter_map(|constraint| match constraint {
            StringConstraint::MaximumLength(maximum) => Some(maximum.length),
            StringConstraint::MinimumLength(_) => None,
        })
        .min();

    if let (Some(minimum), Some(maximum)) = (strongest_minimum, strongest_maximum) {
        if minimum > maximum {
            return Err(schema_invalid(
                value_type,
                SchemaInvalidityKind::ContradictoryConstraints,
                format!(
                    "Effective string length interval is empty: minimum length {minimum}, maximum length {maximum}"
                ),
            ));
        }
    }

    Ok(())
}

fn unsupported_constraint(
    value_type: &HolonReference,
    constraint_holon: &HolonReference,
) -> HolonError {
    schema_invalid(
        value_type,
        SchemaInvalidityKind::UnsupportedExecutableConstraint,
        format!(
            "Constraint {} has no executable runtime implementation for this value descriptor family",
            descriptor_label(constraint_holon)
        ),
    )
}

fn schema_invalid(
    value_type: &HolonReference,
    kind: SchemaInvalidityKind,
    detail: String,
) -> HolonError {
    HolonError::DescriptorSchemaInvalid { kind, detail, descriptor: descriptor_label(value_type) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::transactions::TransactionContext;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, core_value_type_name, new_descriptor_holon,
    };
    use crate::reference_layer::{TransientReference, WritableHolon};
    use std::sync::Arc;
    use type_names::CoreValueTypeName;

    fn assert_schema_invalid<T>(
        result: Result<T, HolonError>,
        expected_kind: SchemaInvalidityKind,
    ) {
        assert!(matches!(
            result,
            Err(HolonError::DescriptorSchemaInvalid { kind, .. }) if kind == expected_kind
        ));
    }

    fn add_extends(
        child: &mut TransientReference,
        parent: &TransientReference,
    ) -> Result<(), HolonError> {
        child.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.clone().into()])?;
        Ok(())
    }

    fn constraint_holon_with_family(
        context: &Arc<TransactionContext>,
        key: &str,
        constraint_type: CoreHolonTypeName,
        family_type: CoreHolonTypeName,
    ) -> Result<TransientReference, HolonError> {
        let family = new_descriptor_holon(
            context,
            &format!("{key}-family"),
            &core_holon_type_name(family_type),
            "Holon",
        )?;
        let mut constraint =
            new_descriptor_holon(context, key, &core_holon_type_name(constraint_type), "Holon")?;
        add_extends(&mut constraint, &family)?;
        Ok(constraint)
    }

    #[test]
    fn integer_resolver_returns_typed_minimum_constraint() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "integer-family",
            &core_holon_type_name(CoreHolonTypeName::IntegerValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumValue),
            "Holon",
        )?;
        minimum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 5_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, true)?;
        add_extends(&mut minimum, &family)?;
        let mut integer_value = new_descriptor_holon(
            &context,
            "integer-value",
            &core_value_type_name(CoreValueTypeName::IntegerValueType),
            "Value",
        )?;
        integer_value
            .add_related_holons(CoreRelationshipTypeName::Constraints, vec![minimum.into()])?;

        assert_eq!(
            resolve_integer_constraints(&integer_value.into())?,
            vec![IntegerConstraint::Minimum(MinimumValueConstraint { value: 5, inclusive: true })]
        );
        Ok(())
    }

    #[test]
    fn resolver_discovers_constraints_inherited_by_value_type() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "integer-family",
            &core_holon_type_name(CoreHolonTypeName::IntegerValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumValue),
            "Holon",
        )?;
        minimum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 5_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, true)?;
        add_extends(&mut minimum, &family)?;

        let mut parent_value =
            new_descriptor_holon(&context, "parent-value", "ParentIntegerValueType", "Value")?;
        parent_value
            .add_related_holons(CoreRelationshipTypeName::Constraints, vec![minimum.into()])?;
        let mut child_value =
            new_descriptor_holon(&context, "child-value", "ChildIntegerValueType", "Value")?;
        add_extends(&mut child_value, &parent_value)?;

        assert_eq!(
            resolve_integer_constraints(&child_value.into())?,
            vec![IntegerConstraint::Minimum(MinimumValueConstraint { value: 5, inclusive: true })]
        );
        Ok(())
    }

    #[test]
    fn string_resolver_rejects_incompatible_integer_family_constraint() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "integer-family",
            &core_holon_type_name(CoreHolonTypeName::IntegerValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumValue),
            "Holon",
        )?;
        add_extends(&mut minimum, &family)?;
        let mut string_value = new_descriptor_holon(
            &context,
            "string-value",
            &core_value_type_name(CoreValueTypeName::StringValueType),
            "Value",
        )?;
        string_value
            .add_related_holons(CoreRelationshipTypeName::Constraints, vec![minimum.into()])?;

        assert_schema_invalid(
            resolve_string_constraints(&string_value.into()),
            SchemaInvalidityKind::IncompatibleConstraintFamily,
        );
        Ok(())
    }

    #[test]
    fn integer_resolver_rejects_incompatible_string_bytes_and_value_array_families(
    ) -> Result<(), HolonError> {
        for (key, constraint_type, family_type) in [
            (
                "string-minimum",
                CoreHolonTypeName::MinimumLength,
                CoreHolonTypeName::StringValueConstraint,
            ),
            (
                "bytes-constraint",
                CoreHolonTypeName::ValueConstraintType,
                CoreHolonTypeName::BytesValueConstraint,
            ),
            (
                "array-constraint",
                CoreHolonTypeName::ValueConstraintType,
                CoreHolonTypeName::ValueArrayConstraint,
            ),
        ] {
            let context = build_context();
            let constraint =
                constraint_holon_with_family(&context, key, constraint_type, family_type)?;
            let mut integer_value = new_descriptor_holon(
                &context,
                "integer-value",
                &core_value_type_name(CoreValueTypeName::IntegerValueType),
                "Value",
            )?;
            integer_value.add_related_holons(
                CoreRelationshipTypeName::Constraints,
                vec![constraint.into()],
            )?;

            assert_schema_invalid(
                resolve_integer_constraints(&integer_value.into()),
                SchemaInvalidityKind::IncompatibleConstraintFamily,
            );
        }
        Ok(())
    }

    #[test]
    fn string_resolver_rejects_incompatible_bytes_and_value_array_families(
    ) -> Result<(), HolonError> {
        for (key, family_type) in [
            ("bytes-constraint", CoreHolonTypeName::BytesValueConstraint),
            ("array-constraint", CoreHolonTypeName::ValueArrayConstraint),
        ] {
            let context = build_context();
            let constraint = constraint_holon_with_family(
                &context,
                key,
                CoreHolonTypeName::ValueConstraintType,
                family_type,
            )?;
            let mut string_value = new_descriptor_holon(
                &context,
                "string-value",
                &core_value_type_name(CoreValueTypeName::StringValueType),
                "Value",
            )?;
            string_value.add_related_holons(
                CoreRelationshipTypeName::Constraints,
                vec![constraint.into()],
            )?;

            assert_schema_invalid(
                resolve_string_constraints(&string_value.into()),
                SchemaInvalidityKind::IncompatibleConstraintFamily,
            );
        }
        Ok(())
    }

    #[test]
    fn integer_resolver_rejects_unclassified_constraint() -> Result<(), HolonError> {
        let context = build_context();
        let mystery = new_descriptor_holon(&context, "mystery", "MysteryConstraint", "Holon")?;
        let mut integer_value = new_descriptor_holon(
            &context,
            "integer-value",
            &core_value_type_name(CoreValueTypeName::IntegerValueType),
            "Value",
        )?;
        integer_value
            .add_related_holons(CoreRelationshipTypeName::Constraints, vec![mystery.into()])?;

        assert_schema_invalid(
            resolve_integer_constraints(&integer_value.into()),
            SchemaInvalidityKind::UnclassifiedConstraint,
        );
        Ok(())
    }

    #[test]
    fn integer_resolver_reports_missing_integer_constraint_parameter() -> Result<(), HolonError> {
        let context = build_context();
        let minimum = constraint_holon_with_family(
            &context,
            "minimum",
            CoreHolonTypeName::MinimumValue,
            CoreHolonTypeName::IntegerValueConstraint,
        )?;
        let mut integer_value = new_descriptor_holon(
            &context,
            "integer-value",
            &core_value_type_name(CoreValueTypeName::IntegerValueType),
            "Value",
        )?;
        integer_value
            .add_related_holons(CoreRelationshipTypeName::Constraints, vec![minimum.into()])?;

        assert_schema_invalid(
            resolve_integer_constraints(&integer_value.into()),
            SchemaInvalidityKind::MissingConstraintParameter,
        );
        Ok(())
    }

    #[test]
    fn integer_resolver_detects_empty_exclusive_interval() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "integer-family",
            &core_holon_type_name(CoreHolonTypeName::IntegerValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumValue),
            "Holon",
        )?;
        minimum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 5_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, false)?;
        add_extends(&mut minimum, &family)?;
        let mut maximum = new_descriptor_holon(
            &context,
            "maximum",
            &core_holon_type_name(CoreHolonTypeName::MaximumValue),
            "Holon",
        )?;
        maximum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 6_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, false)?;
        add_extends(&mut maximum, &family)?;
        let mut integer_value = new_descriptor_holon(
            &context,
            "integer-value",
            &core_value_type_name(CoreValueTypeName::IntegerValueType),
            "Value",
        )?;
        integer_value.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![minimum.into(), maximum.into()],
        )?;

        assert_schema_invalid(
            resolve_integer_constraints(&integer_value.into()),
            SchemaInvalidityKind::ContradictoryConstraints,
        );
        Ok(())
    }

    #[test]
    fn string_resolver_detects_contradictory_length_interval() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "string-family",
            &core_holon_type_name(CoreHolonTypeName::StringValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumLength),
            "Holon",
        )?;
        minimum.with_property_value(CorePropertyTypeName::ConstraintLength, 10_i64)?;
        add_extends(&mut minimum, &family)?;
        let mut maximum = new_descriptor_holon(
            &context,
            "maximum",
            &core_holon_type_name(CoreHolonTypeName::MaximumLength),
            "Holon",
        )?;
        maximum.with_property_value(CorePropertyTypeName::ConstraintLength, 5_i64)?;
        add_extends(&mut maximum, &family)?;
        let mut string_value = new_descriptor_holon(
            &context,
            "string-value",
            &core_value_type_name(CoreValueTypeName::StringValueType),
            "Value",
        )?;
        string_value.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![minimum.into(), maximum.into()],
        )?;

        assert_schema_invalid(
            resolve_string_constraints(&string_value.into()),
            SchemaInvalidityKind::ContradictoryConstraints,
        );
        Ok(())
    }

    #[test]
    fn string_constraint_validation_counts_unicode_scalar_values() -> Result<(), HolonError> {
        let minimum = StringConstraint::MinimumLength(MinimumLengthConstraint { length: 1 });

        assert!(minimum.is_valid("\u{e9}", "unicode-string").is_ok());
        Ok(())
    }

    #[test]
    fn missing_constraint_parameter_is_schema_invalid() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "string-family",
            &core_holon_type_name(CoreHolonTypeName::StringValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumLength),
            "Holon",
        )?;
        add_extends(&mut minimum, &family)?;
        let mut string_value = new_descriptor_holon(
            &context,
            "string-value",
            &core_value_type_name(CoreValueTypeName::StringValueType),
            "Value",
        )?;
        string_value
            .add_related_holons(CoreRelationshipTypeName::Constraints, vec![minimum.into()])?;

        assert_schema_invalid(
            resolve_string_constraints(&string_value.into()),
            SchemaInvalidityKind::MissingConstraintParameter,
        );
        Ok(())
    }
}
