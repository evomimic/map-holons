use super::{Bound, IntegerInterval, MeasureBounds, ScalarConstraintParameters};
use base_types::BaseValue;
use std::fmt;
use unicode_segmentation::UnicodeSegmentation;

/// Stable semantic code for a violated string grapheme-length constraint.
pub const STRING_LENGTH_OUT_OF_RANGE: &str = "StringLengthOutOfRange";
/// Stable semantic code for a violated byte-length constraint.
pub const BYTES_LENGTH_OUT_OF_RANGE: &str = "BytesLengthOutOfRange";
/// Stable semantic code for a violated signed numeric-range constraint.
pub const NUMERIC_VALUE_OUT_OF_RANGE: &str = "NumericValueOutOfRange";

/// A decline leaves representation diagnosis to the consumer; it is never a pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintEvaluationOutcome {
    Satisfied,
    Violated(ConstraintViolationDetail),
    IncompatibleValueKind,
}

/// An observed scalar value or non-negative length, without widening signed integers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintMeasure {
    Length(u64),
    Integer(i64),
}

impl fmt::Display for ConstraintMeasure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(value) => value.fmt(f),
            Self::Integer(value) => value.fmt(f),
        }
    }
}

/// Pure evaluation evidence. Consumers attach subject and descriptor provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintViolationDetail {
    pub code: &'static str,
    pub measure: ConstraintMeasure,
    pub bounds: ScalarConstraintParameters,
}

impl fmt::Display for ConstraintViolationDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.bounds {
            ScalarConstraintParameters::StringLength(bounds) => {
                write!(f, "measured {} graphemes; required ", self.measure)?;
                display_measure_interval(f, bounds)
            }
            ScalarConstraintParameters::BytesLength(bounds) => {
                write!(f, "measured {} bytes; required ", self.measure)?;
                display_measure_interval(f, bounds)
            }
            ScalarConstraintParameters::NumericRange(bounds) => {
                write!(f, "measured {}; required ", self.measure)?;
                display_interval(f, bounds.minimum, bounds.maximum)
            }
        }
    }
}

/// Lengths have a natural inclusive floor even when no minimum was authored.
fn display_measure_interval(f: &mut fmt::Formatter<'_>, bounds: &MeasureBounds) -> fmt::Result {
    let minimum = bounds.minimum.or(Some(Bound { value: 0, inclusive: true }));
    display_interval(f, minimum, bounds.maximum)
}

fn display_interval<T: fmt::Display>(
    f: &mut fmt::Formatter<'_>,
    minimum: Option<Bound<T>>,
    maximum: Option<Bound<T>>,
) -> fmt::Result {
    match minimum {
        Some(bound) => write!(f, "{}{}", if bound.inclusive { '[' } else { '(' }, bound.value)?,
        None => write!(f, "(-inf")?,
    }
    match maximum {
        Some(bound) => write!(f, ", {}{}", bound.value, if bound.inclusive { ']' } else { ')' }),
        None => write!(f, ", +inf)"),
    }
}

/// Compares endpoints directly: exclusive extrema and empty integer intervals need no arithmetic.
fn within_bounds<T: PartialOrd>(
    actual: T,
    minimum: &Option<Bound<T>>,
    maximum: &Option<Bound<T>>,
) -> bool {
    minimum.as_ref().is_none_or(|bound| {
        if bound.inclusive {
            actual >= bound.value
        } else {
            actual > bound.value
        }
    }) && maximum.as_ref().is_none_or(|bound| {
        if bound.inclusive {
            actual <= bound.value
        } else {
            actual < bound.value
        }
    })
}

/// Counts default extended grapheme clusters on the stored string without normalization.
fn evaluate_string_length(
    bounds: &MeasureBounds,
    value: &BaseValue,
) -> ConstraintEvaluationOutcome {
    let BaseValue::StringValue(value) = value else {
        return ConstraintEvaluationOutcome::IncompatibleValueKind;
    };
    let measured = value.0.graphemes(true).count() as u64;
    evaluate_measure(
        bounds,
        measured,
        STRING_LENGTH_OUT_OF_RANGE,
        ScalarConstraintParameters::StringLength,
    )
}

/// Counts bytes only for the native byte representation.
fn evaluate_bytes_length(bounds: &MeasureBounds, value: &BaseValue) -> ConstraintEvaluationOutcome {
    let BaseValue::BytesValue(value) = value else {
        return ConstraintEvaluationOutcome::IncompatibleValueKind;
    };
    evaluate_measure(
        bounds,
        value.0.len() as u64,
        BYTES_LENGTH_OUT_OF_RANGE,
        ScalarConstraintParameters::BytesLength,
    )
}

fn evaluate_measure(
    bounds: &MeasureBounds,
    measured: u64,
    code: &'static str,
    parameters: fn(MeasureBounds) -> ScalarConstraintParameters,
) -> ConstraintEvaluationOutcome {
    if within_bounds(measured, &bounds.minimum, &bounds.maximum) {
        ConstraintEvaluationOutcome::Satisfied
    } else {
        ConstraintEvaluationOutcome::Violated(ConstraintViolationDetail {
            code,
            measure: ConstraintMeasure::Length(measured),
            bounds: parameters(*bounds),
        })
    }
}

/// Evaluates signed integer endpoints without incrementing or decrementing them.
fn evaluate_numeric_range(
    bounds: &IntegerInterval,
    value: &BaseValue,
) -> ConstraintEvaluationOutcome {
    let BaseValue::IntegerValue(value) = value else {
        return ConstraintEvaluationOutcome::IncompatibleValueKind;
    };
    if within_bounds(value.0, &bounds.minimum, &bounds.maximum) {
        ConstraintEvaluationOutcome::Satisfied
    } else {
        ConstraintEvaluationOutcome::Violated(ConstraintViolationDetail {
            code: NUMERIC_VALUE_OUT_OF_RANGE,
            measure: ConstraintMeasure::Integer(value.0),
            bounds: ScalarConstraintParameters::NumericRange(*bounds),
        })
    }
}

impl ScalarConstraintParameters {
    /// Parameters select their own evaluator, so dispatch cannot pair different families.
    pub fn evaluate(&self, value: &BaseValue) -> ConstraintEvaluationOutcome {
        match self {
            Self::StringLength(bounds) => evaluate_string_length(bounds, value),
            Self::BytesLength(bounds) => evaluate_bytes_length(bounds, value),
            Self::NumericRange(bounds) => evaluate_numeric_range(bounds, value),
        }
    }
}
