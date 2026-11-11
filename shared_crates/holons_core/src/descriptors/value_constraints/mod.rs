//! Shared configuration interpretation for descriptor-backed value and measured constraints.
mod configuration;
mod evaluators;
mod interpretation;
mod parameters;
mod support;

pub use configuration::{
    parse_constraint_configuration, ConfigurationDiagnosis, ConstraintConfiguration,
};
pub use evaluators::{
    ConstraintEvaluationOutcome, ConstraintMeasure, ConstraintViolationDetail,
    BYTES_LENGTH_OUT_OF_RANGE, NUMERIC_VALUE_OUT_OF_RANGE, STRING_LENGTH_OUT_OF_RANGE,
};
pub use interpretation::{
    classify_constraint_type, interpret_constraint, ClassifiedConstraintType,
    ConstraintInterpretation, ConstraintTypeDefect,
};
pub use parameters::{
    Bound, ConstraintParameters, IntegerInterval, MeasureBounds, ScalarConstraintParameters,
};
pub use support::{ConstraintFamily, ScalarConstraintFamily};

#[cfg(test)]
mod tests;
