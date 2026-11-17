use crate::descriptors::test_support::{
    build_context, build_context_with_read_errors, new_holon_type_descriptor, new_kind_descriptor,
    new_schema_holon,
};
use crate::descriptors::{Descriptor, ValueDescriptor};
use crate::{HolonReference, WritableHolon};
use base_types::{BaseValue, MapBytes, MapInteger};
use core_types::{HolonError, HolonId, LocalId, SchemaInvalidityKind};
use std::collections::HashMap;
use type_names::CoreRelationshipTypeName as R;

fn constrained(kind: &str, family: &str) -> (ValueDescriptor, HolonReference, HolonReference) {
    let context = build_context();
    let root: HolonReference =
        new_kind_descriptor(&context, "TypeDescriptor", None, false).unwrap().into();
    let value_type: HolonReference =
        new_kind_descriptor(&context, kind, Some(&root), true).unwrap().into();
    let mut anchor =
        new_holon_type_descriptor(&context, "ConstraintType.HolonType", "ConstraintType").unwrap();
    anchor.add_related_holons(R::Extends, vec![root]).unwrap();
    let mut constraint_type =
        new_holon_type_descriptor(&context, &format!("{family}.ConstraintType"), family).unwrap();
    constraint_type.add_related_holons(R::Extends, vec![anchor.into()]).unwrap();
    constraint_type
        .add_related_holons(R::ApplicableToDescriptorTypes, vec![value_type.clone()])
        .unwrap();
    let constraint_type: HolonReference = constraint_type.into();
    let mut constraint: HolonReference =
        new_schema_holon(&context, "bound", &constraint_type).unwrap().into();
    constraint
        .with_property_value("Maximum", 2_i64)
        .unwrap()
        .with_property_value("MaximumIsInclusive", true)
        .unwrap();
    value_type.clone().add_related_holons(R::Constraints, vec![constraint.clone()]).unwrap();
    (ValueDescriptor::from_holon(value_type), constraint, constraint_type)
}

#[test]
fn scalar_literal_entry_point_preserves_family_codes_and_identity_provenance() {
    for (kind, family, code, accepted, rejected) in [
        (
            "StringValueType",
            "StringLengthConstraint",
            "StringLengthOutOfRange",
            BaseValue::StringValue("a\u{301}b".into()),
            BaseValue::StringValue("abc".into()),
        ),
        (
            "BytesValueType",
            "BytesLengthConstraint",
            "BytesLengthOutOfRange",
            BaseValue::BytesValue(MapBytes(vec![1, 2])),
            BaseValue::BytesValue(MapBytes(vec![1, 2, 3])),
        ),
        (
            "IntegerValueType",
            "NumericRangeConstraint",
            "NumericValueOutOfRange",
            BaseValue::IntegerValue(MapInteger(-2)),
            BaseValue::IntegerValue(MapInteger(-3)),
        ),
    ] {
        let (descriptor, mut bound, constraint_type) = constrained(kind, family);
        if family == "NumericRangeConstraint" {
            bound
                .with_property_value("Minimum", -2_i64)
                .unwrap()
                .with_property_value("MinimumIsInclusive", true)
                .unwrap();
        }
        descriptor.validate_literal(&accepted).unwrap();
        let error = descriptor.validate_literal(&rejected).unwrap_err();
        assert!(
            matches!(&error, HolonError::ValueConstraintViolation {
            code: actual_code, value_type, constraint, constraint_type: actual_type, descriptor: declaring, detail,
        } if actual_code == code && value_type == &descriptor.holon().reference_id_string()
            && constraint == &bound.reference_id_string() && actual_type == &constraint_type.reference_id_string()
            && declaring == value_type && detail.contains("measured") && detail.contains("required")),
            "{error:?}"
        );
        assert_eq!(
            crate::dances::ResponseStatusCode::from(error),
            crate::dances::ResponseStatusCode::UnprocessableEntity
        );
    }
}

#[test]
fn a_scalar_decline_under_any_base_value_is_a_kind_error() {
    let (descriptor, _, _) = constrained("BaseValueValueType", "StringLengthConstraint");
    assert!(
        matches!(descriptor.validate_literal(&BaseValue::IntegerValue(MapInteger(1))), Err(HolonError::ValueKindMismatch { expected, found, .. }) if expected == "String" && found == "Integer")
    );
}

#[test]
fn representation_validation_precedes_constraints_and_arrays_remain_explicitly_unsupported() {
    let (string, _, _) = constrained("StringValueType", "Unknown");
    assert!(matches!(
        string.validate_literal(&BaseValue::IntegerValue(MapInteger(1))),
        Err(HolonError::ValueKindMismatch { .. })
    ));
    let (array, _, _) = constrained("ValueArrayValueType", "ItemCountConstraint");
    assert!(matches!(
        array.validate_literal(&BaseValue::IntegerValue(MapInteger(1))),
        Err(HolonError::NotImplemented(_))
    ));
}

#[test]
fn extension_constraints_inherit_configuration_semantics_without_executable_support() {
    for valid in [false, true] {
        let context = build_context();
        let root: HolonReference =
            new_kind_descriptor(&context, "TypeDescriptor", None, false).unwrap().into();
        let mut anchor =
            new_holon_type_descriptor(&context, "ConstraintType.HolonType", "ConstraintType")
                .unwrap();
        anchor.add_related_holons(R::Extends, vec![root.clone()]).unwrap();
        let mut family = new_holon_type_descriptor(
            &context,
            "StringLengthConstraint.ConstraintType",
            "StringLengthConstraint",
        )
        .unwrap();
        family.add_related_holons(R::Extends, vec![anchor.into()]).unwrap();
        let mut extension =
            new_holon_type_descriptor(&context, "Extension.ConstraintType", "Extension").unwrap();
        extension.add_related_holons(R::Extends, vec![family.into()]).unwrap();
        let mut value_type =
            new_kind_descriptor(&context, "StringValueType", Some(&root), true).unwrap();
        extension
            .add_related_holons(R::ApplicableToDescriptorTypes, vec![value_type.clone().into()])
            .unwrap();
        let mut bound = new_schema_holon(&context, "extension-bound", &extension.into()).unwrap();
        if valid {
            bound
                .with_property_value("Maximum", 2_i64)
                .unwrap()
                .with_property_value("MaximumIsInclusive", true)
                .unwrap();
        }
        value_type.add_related_holons(R::Constraints, vec![bound.into()]).unwrap();
        let error = ValueDescriptor::from_holon(value_type.into())
            .validate_literal(&BaseValue::StringValue("a".into()))
            .unwrap_err();
        if valid {
            assert!(matches!(error, HolonError::UnsupportedValueConstraint { .. }), "{error:?}");
        } else {
            assert!(
                matches!(
                    error,
                    HolonError::DescriptorSchemaInvalid {
                        kind: SchemaInvalidityKind::InvalidConstraintConfiguration,
                        ..
                    }
                ),
                "{error:?}"
            );
        }
    }
}

#[test]
fn acquisition_errors_are_not_reclassified_as_schema_or_constraint_errors() {
    for error in [
        HolonError::EmptyField("backend read failed".into()),
        HolonError::CacheError("unavailable schema".into()),
    ] {
        let id = HolonId::Local(LocalId(vec![51; 39]));
        let context = build_context_with_read_errors(
            vec![],
            HashMap::new(),
            HashMap::from([(id.clone(), error.clone())]),
        );
        let root: HolonReference =
            new_kind_descriptor(&context, "TypeDescriptor", None, false).unwrap().into();
        let mut value_type =
            new_kind_descriptor(&context, "StringValueType", Some(&root), true).unwrap();
        let unreadable: HolonReference = crate::SmartReference::new_with_properties(
            context.space_read_handle(),
            id,
            core_types::PropertyMap::from([(
                type_names::ToPropertyName::to_property_name(type_names::CorePropertyTypeName::Key),
                BaseValue::StringValue("unreadable-type".into()),
            )]),
        )
        .into();
        let bound = new_schema_holon(&context, "unreadable-bound", &unreadable).unwrap();
        value_type.add_related_holons(R::Constraints, vec![bound.into()]).unwrap();
        let result = ValueDescriptor::from_holon(value_type.into())
            .validate_literal(&BaseValue::StringValue("a".into()));
        assert_eq!(result, Err(error));
    }
}

#[test]
fn unrecognized_families_have_no_invented_core_configuration_requirements() {
    let (descriptor, mut bound, _) = constrained("StringValueType", "Unknown");
    bound
        .with_property_value("Maximum", "extension-owned")
        .unwrap()
        .with_property_value("MinimumIsInclusive", 7_i64)
        .unwrap();
    assert!(matches!(
        descriptor.validate_literal(&BaseValue::StringValue("a".into())),
        Err(HolonError::UnsupportedValueConstraint { .. })
    ));
}
