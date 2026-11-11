use super::*;
use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use type_names::CoreDefinitionRole as Role;

const BOUNDED: [ConstraintFamily; 4] = [
    ConstraintFamily::StringLength,
    ConstraintFamily::BytesLength,
    ConstraintFamily::NumericRange,
    ConstraintFamily::ItemCount,
];

fn integer(value: i64) -> BaseValue {
    BaseValue::IntegerValue(MapInteger(value))
}

fn boolean(value: bool) -> BaseValue {
    BaseValue::BooleanValue(MapBoolean(value))
}

fn configuration(
    minimum: Option<i64>,
    maximum: Option<i64>,
    lower: Option<bool>,
    upper: Option<bool>,
) -> ConstraintConfiguration {
    ConstraintConfiguration {
        minimum: minimum.map(integer),
        maximum: maximum.map(integer),
        minimum_is_inclusive: lower.map(boolean),
        maximum_is_inclusive: upper.map(boolean),
    }
}

#[test]
fn bounded_family_configuration_matrix() {
    for family in BOUNDED {
        for (min, max, lower, upper, valid) in [
            (None, None, None, None, false),
            (Some(0), None, Some(true), None, true),
            (Some(0), None, Some(false), None, true),
            (None, Some(8), None, Some(false), true),
            (None, Some(8), None, Some(true), true),
            (Some(2), Some(8), Some(false), Some(false), true),
            (Some(2), Some(2), Some(true), Some(true), true),
            (Some(2), Some(2), Some(false), Some(true), false),
            (Some(2), Some(2), Some(true), Some(false), false),
            (Some(2), Some(2), Some(false), Some(false), false),
            (Some(8), Some(2), Some(true), Some(true), false),
            // Ordered intervals are declarable even when no integer can satisfy them.
            (Some(0), Some(1), Some(false), Some(false), true),
            (Some(0), None, None, None, false),
            (None, Some(8), None, None, false),
            (None, Some(8), Some(true), Some(true), false),
            (Some(0), None, Some(true), Some(true), false),
        ] {
            let result =
                parse_constraint_configuration(family, &configuration(min, max, lower, upper));
            assert_eq!(
                result.is_ok(),
                valid,
                "{family:?}: {min:?} {max:?} {lower:?} {upper:?}: {result:?}"
            );
        }
    }
}

#[test]
fn signed_bounds_and_integer_extrema_are_preserved_without_arithmetic() {
    for (min, max) in [(i64::MIN, i64::MAX), (-5, -1), (i64::MIN, i64::MIN), (i64::MAX, i64::MAX)] {
        let parameters = parse_constraint_configuration(
            ConstraintFamily::NumericRange,
            &configuration(Some(min), Some(max), Some(true), Some(true)),
        )
        .unwrap();
        assert_eq!(
            parameters,
            ConstraintParameters::NumericRange(IntegerInterval {
                minimum: Some(Bound { value: min, inclusive: true }),
                maximum: Some(Bound { value: max, inclusive: true }),
            })
        );
    }
    for bounds in [
        configuration(Some(-1), None, Some(false), None),
        configuration(None, Some(-1), None, Some(false)),
    ] {
        assert!(parse_constraint_configuration(ConstraintFamily::NumericRange, &bounds).is_ok());
        for family in [
            ConstraintFamily::StringLength,
            ConstraintFamily::BytesLength,
            ConstraintFamily::ItemCount,
        ] {
            let errors = parse_constraint_configuration(family, &bounds).unwrap_err();
            assert_eq!(errors.len(), 1);
            assert!(errors[0].message.contains("must be non-negative; found integer -1"));
        }
    }
}

#[test]
fn typed_measure_parameters_keep_family_and_inclusivity() {
    let bounds = MeasureBounds {
        minimum: Some(Bound { value: 0, inclusive: false }),
        maximum: Some(Bound { value: i64::MAX as u64, inclusive: true }),
    };
    for (family, expected) in [
        (ConstraintFamily::StringLength, ConstraintParameters::StringLength(bounds.clone())),
        (ConstraintFamily::BytesLength, ConstraintParameters::BytesLength(bounds.clone())),
        (ConstraintFamily::ItemCount, ConstraintParameters::ItemCount(bounds)),
    ] {
        assert_eq!(
            parse_constraint_configuration(
                family,
                &configuration(Some(0), Some(i64::MAX), Some(false), Some(true))
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn configuration_diagnoses_accumulate_without_inventing_interval_failures() {
    for family in BOUNDED {
        let config = ConstraintConfiguration {
            minimum: Some(BaseValue::StringValue(MapString("bad".into()))),
            maximum: Some(boolean(true)),
            minimum_is_inclusive: Some(integer(1)),
            maximum_is_inclusive: Some(integer(0)),
        };
        let errors = parse_constraint_configuration(family, &config).unwrap_err();
        assert_eq!(errors.len(), 4);
        assert!(errors[0].message.contains("Minimum must be an integer; found value kind String"));
        assert!(errors[1].message.contains("Maximum must be an integer"));
        assert_eq!(errors[2].message, "MinimumIsInclusive must be Boolean.");
        assert_eq!(errors[3].message, "MaximumIsInclusive must be Boolean.");
        let mut config = configuration(Some(2), Some(2), Some(true), Some(true));
        config.minimum_is_inclusive = Some(integer(0));
        let errors = parse_constraint_configuration(family, &config).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "MinimumIsInclusive must be Boolean.");
    }
}

#[test]
fn cardinality_requires_nonnegative_minimum_and_has_no_flags() {
    for (min, max, valid) in [
        (None, None, false),
        (None, Some(2), false),
        (Some(-1), None, false),
        (Some(0), Some(-1), false),
        (Some(2), Some(1), false),
        (Some(2), Some(2), true),
        (Some(0), None, true),
        (Some(0), Some(i64::MAX), true),
    ] {
        assert_eq!(
            parse_constraint_configuration(
                ConstraintFamily::Cardinality,
                &configuration(min, max, None, None)
            )
            .is_ok(),
            valid
        );
    }
    assert_eq!(
        parse_constraint_configuration(
            ConstraintFamily::Cardinality,
            &configuration(Some(2), None, None, None)
        )
        .unwrap(),
        ConstraintParameters::Cardinality { minimum: 2, maximum: None }
    );
    for (lower, upper) in [(Some(false), None), (None, Some(true)), (Some(true), Some(true))] {
        let errors = parse_constraint_configuration(
            ConstraintFamily::Cardinality,
            &configuration(Some(0), None, lower, upper),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("admits no inclusivity flags"));
    }
    let config = ConstraintConfiguration {
        minimum: Some(boolean(true)),
        maximum: Some(boolean(false)),
        minimum_is_inclusive: Some(integer(1)),
        maximum_is_inclusive: None,
    };
    assert_eq!(
        parse_constraint_configuration(ConstraintFamily::Cardinality, &config).unwrap_err().len(),
        4
    );
}

#[test]
fn unique_items_is_presence_only_regardless_of_native_kind() {
    assert_eq!(
        parse_constraint_configuration(
            ConstraintFamily::UniqueItems,
            &ConstraintConfiguration::default()
        )
        .unwrap(),
        ConstraintParameters::UniqueItems
    );
    let errors = parse_constraint_configuration(
        ConstraintFamily::UniqueItems,
        &ConstraintConfiguration {
            minimum: Some(integer(-1)),
            maximum: Some(boolean(false)),
            minimum_is_inclusive: Some(integer(1)),
            maximum_is_inclusive: Some(boolean(false)),
        },
    )
    .unwrap_err();
    assert_eq!(errors.len(), 4);
    for (error, name) in
        errors.iter().zip(["Minimum", "Maximum", "MinimumIsInclusive", "MaximumIsInclusive"])
    {
        assert_eq!(
            error.message,
            format!("UniqueItemsConstraint is presence-only and has no {name} parameter.")
        );
    }
}

#[test]
fn structural_defects_take_precedence_over_configuration() {
    for defect in [
        ConstraintTypeDefect::MissingDescribedBy,
        ConstraintTypeDefect::MultipleDescribedBy(2),
        ConstraintTypeDefect::UnusableLineage,
        ConstraintTypeDefect::NotConcreteConstraintType,
    ] {
        assert_eq!(
            interpret_constraint(Err(defect.clone()), &ConstraintConfiguration::default()),
            ConstraintInterpretation::StructurallyInvalid(defect)
        );
    }
}

#[test]
fn executable_support_requires_exact_scalar_type_keys() {
    for (role, family) in [
        (Role::StringLengthConstraint, ScalarConstraintFamily::StringLength),
        (Role::BytesLengthConstraint, ScalarConstraintFamily::BytesLength),
        (Role::NumericRangeConstraint, ScalarConstraintFamily::NumericRange),
    ] {
        assert_eq!(ScalarConstraintFamily::for_concrete_type(role.key()), Some(family));
    }
    for key in [
        Role::ItemCountConstraint.key(),
        Role::CardinalityConstraint.key(),
        Role::UniqueItemsConstraint.key(),
        "Extension.StringLengthConstraint",
        "unknown",
    ] {
        assert_eq!(ScalarConstraintFamily::for_concrete_type(key), None);
    }
}
