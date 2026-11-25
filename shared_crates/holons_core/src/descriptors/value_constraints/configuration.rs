use super::{Bound, ConstraintFamily, ConstraintParameters, IntegerInterval, MeasureBounds};
use crate::{HolonReference, ReadableHolon};
use base_types::BaseValue;
use core_types::HolonError;
use type_names::CorePropertyTypeName as P;

/// Authored fields retain presence and native kind until the pure parser interprets them.
#[derive(Clone, Debug, Default)]
pub struct ConstraintConfiguration {
    pub minimum: Option<BaseValue>,
    pub maximum: Option<BaseValue>,
    pub minimum_is_inclusive: Option<BaseValue>,
    pub maximum_is_inclusive: Option<BaseValue>,
}

impl ConstraintConfiguration {
    /// Reads configuration once through the bound reference. Backend failures remain errors.
    pub fn acquire(constraint: &HolonReference) -> Result<Self, HolonError> {
        Ok(Self {
            minimum: constraint.property_value(P::Minimum)?,
            maximum: constraint.property_value(P::Maximum)?,
            minimum_is_inclusive: constraint.property_value(P::MinimumIsInclusive)?,
            maximum_is_inclusive: constraint.property_value(P::MaximumIsInclusive)?,
        })
    }
}

/// One authored configuration defect, without subject or Commit attribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigurationDiagnosis {
    pub message: String,
}

/// Interprets all four fields and returns every independently diagnosable family defect.
/// It performs no schema reads and does not require an executable evaluator.
pub fn parse_constraint_configuration(
    family: ConstraintFamily,
    configuration: &ConstraintConfiguration,
) -> Result<ConstraintParameters, Vec<ConfigurationDiagnosis>> {
    match family {
        ConstraintFamily::UniqueItems => {
            parse_presence_only("UniqueItemsConstraint", configuration)
                .map(|()| ConstraintParameters::UniqueItems)
        }
        ConstraintFamily::PositionalStringTemplateSyntax => {
            parse_presence_only("PositionalStringTemplateSyntaxConstraint", configuration)
                .map(|()| ConstraintParameters::PositionalStringTemplateSyntax)
        }
        ConstraintFamily::Cardinality => parse_bounded(configuration, BoundsKind::Cardinality)
            .and_then(|bounds| {
                let minimum = bounds.minimum.ok_or_else(|| {
                    vec![ConfigurationDiagnosis {
                        message: "CardinalityConstraint requires Minimum.".into(),
                    }]
                })?;
                Ok(ConstraintParameters::Cardinality {
                    minimum: minimum.value as u64,
                    maximum: bounds.maximum.map(|bound| bound.value as u64),
                })
            }),
        ConstraintFamily::NumericRange => {
            parse_bounded(configuration, BoundsKind::Signed).map(ConstraintParameters::NumericRange)
        }
        ConstraintFamily::StringLength => {
            parse_measure_bounds(configuration).map(ConstraintParameters::StringLength)
        }
        ConstraintFamily::BytesLength => {
            parse_measure_bounds(configuration).map(ConstraintParameters::BytesLength)
        }
        ConstraintFamily::ItemCount => {
            parse_measure_bounds(configuration).map(ConstraintParameters::ItemCount)
        }
    }
}

#[derive(PartialEq, Eq)]
enum BoundsKind {
    Signed,
    NonNegative,
    Cardinality,
}

fn parse_measure_bounds(
    configuration: &ConstraintConfiguration,
) -> Result<MeasureBounds, Vec<ConfigurationDiagnosis>> {
    let interval = parse_bounded(configuration, BoundsKind::NonNegative)?;
    let measure =
        |bound: Bound<i64>| Bound { value: bound.value as u64, inclusive: bound.inclusive };
    Ok(MeasureBounds {
        minimum: interval.minimum.map(measure),
        maximum: interval.maximum.map(measure),
    })
}

fn parse_bounded(
    configuration: &ConstraintConfiguration,
    kind: BoundsKind,
) -> Result<IntegerInterval, Vec<ConfigurationDiagnosis>> {
    let mut diagnoses = Vec::new();
    let mut diagnose = |message: String| diagnoses.push(ConfigurationDiagnosis { message });

    let signed = kind == BoundsKind::Signed;
    let minimum = integer(P::Minimum, &configuration.minimum, signed, &mut diagnose);
    let maximum = integer(P::Maximum, &configuration.maximum, signed, &mut diagnose);
    let lower = boolean(P::MinimumIsInclusive, &configuration.minimum_is_inclusive, &mut diagnose);
    let upper = boolean(P::MaximumIsInclusive, &configuration.maximum_is_inclusive, &mut diagnose);
    let cardinality = kind == BoundsKind::Cardinality;
    if cardinality {
        if configuration.minimum.is_none() {
            diagnose("CardinalityConstraint requires Minimum.".into());
        }
        if configuration.minimum_is_inclusive.is_some()
            || configuration.maximum_is_inclusive.is_some()
        {
            diagnose(
                "CardinalityConstraint has inclusive bounds and admits no inclusivity flags."
                    .into(),
            );
        }
    } else {
        if configuration.minimum.is_none() && configuration.maximum.is_none() {
            diagnose("At least one of Minimum and Maximum is required.".into());
        }
        for (bound, flag, name) in [
            (
                configuration.minimum.is_some(),
                configuration.minimum_is_inclusive.is_some(),
                "MinimumIsInclusive",
            ),
            (
                configuration.maximum.is_some(),
                configuration.maximum_is_inclusive.is_some(),
                "MaximumIsInclusive",
            ),
        ] {
            if bound != flag {
                diagnose(format!("{name} must be present exactly when its bound is present."));
            }
        }
    }
    // Malformed flags already have diagnoses; do not invent a second equal-endpoint defect.
    if let (Some(minimum), Some(maximum)) = (minimum, maximum) {
        if minimum > maximum
            || (minimum == maximum
                && !cardinality
                && matches!((lower, upper), (Some(lower), Some(upper)) if !lower || !upper))
        {
            diagnose(
                "Bounds form an invalid interval; equal endpoints require both ends inclusive."
                    .into(),
            );
        }
    }
    if !diagnoses.is_empty() {
        return Err(diagnoses);
    }
    if cardinality {
        let inclusive_bound = |value| Bound { value, inclusive: true };
        return Ok(IntegerInterval {
            minimum: minimum.map(inclusive_bound),
            maximum: maximum.map(inclusive_bound),
        });
    }
    Ok(IntegerInterval {
        minimum: configured_bound(minimum, lower, "MinimumIsInclusive")?,
        maximum: configured_bound(maximum, upper, "MaximumIsInclusive")?,
    })
}

/// Rejects authored bounds and flags. No Boolean parameter controls this commitment.
/// Property membership checks other undeclared fields, including enable/disable flags.
fn parse_presence_only(
    family_name: &str,
    configuration: &ConstraintConfiguration,
) -> Result<(), Vec<ConfigurationDiagnosis>> {
    let diagnoses: Vec<_> = [
        (P::Minimum, &configuration.minimum),
        (P::Maximum, &configuration.maximum),
        (P::MinimumIsInclusive, &configuration.minimum_is_inclusive),
        (P::MaximumIsInclusive, &configuration.maximum_is_inclusive),
    ]
    .into_iter()
    .filter(|(_, value)| value.is_some())
    .map(|(field, _)| ConfigurationDiagnosis {
        message: format!(
            "{family_name} is presence-only and has no {} parameter.",
            field.as_property_name()
        ),
    })
    .collect();
    if diagnoses.is_empty() {
        Ok(())
    } else {
        Err(diagnoses)
    }
}

/// Builds an endpoint only from a complete bound/flag pair, retaining fail-closed construction
/// even if the preceding diagnosis logic changes.
fn configured_bound(
    value: Option<i64>,
    inclusive: Option<bool>,
    flag_name: &str,
) -> Result<Option<Bound<i64>>, Vec<ConfigurationDiagnosis>> {
    match (value, inclusive) {
        (Some(value), Some(inclusive)) => Ok(Some(Bound { value, inclusive })),
        (None, None) => Ok(None),
        _ => Err(vec![ConfigurationDiagnosis {
            message: format!("{flag_name} must be present exactly when its bound is present."),
        }]),
    }
}

fn integer(
    field: P,
    value: &Option<BaseValue>,
    signed: bool,
    diagnose: &mut impl FnMut(String),
) -> Option<i64> {
    match value {
        Some(BaseValue::IntegerValue(value)) if signed || value.0 >= 0 => Some(value.0),
        Some(BaseValue::IntegerValue(value)) => {
            diagnose(format!(
                "{} must be non-negative; found integer {}.",
                field.as_property_name(),
                value.0
            ));
            None
        }
        Some(value) => {
            diagnose(format!(
                "{} must be an integer; found value kind {:?}.",
                field.as_property_name(),
                value.kind()
            ));
            None
        }
        None => None,
    }
}

fn boolean(field: P, value: &Option<BaseValue>, diagnose: &mut impl FnMut(String)) -> Option<bool> {
    match value {
        Some(BaseValue::BooleanValue(value)) => Some(value.0),
        None => None,
        _ => {
            diagnose(format!("{} must be Boolean.", field.as_property_name()));
            None
        }
    }
}
