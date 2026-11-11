//! Shared configuration interpretation for descriptor-backed value and measured constraints.
mod configuration;
mod interpretation;
mod parameters;
mod support;

pub use configuration::{
    parse_constraint_configuration, ConfigurationDiagnosis, ConstraintConfiguration,
};
pub use interpretation::{interpret_constraint, ConstraintInterpretation, ConstraintTypeDefect};
pub use parameters::{Bound, ConstraintParameters, IntegerInterval, MeasureBounds};
pub use support::{ConstraintFamily, ScalarConstraintFamily};

#[cfg(test)]
mod tests;
