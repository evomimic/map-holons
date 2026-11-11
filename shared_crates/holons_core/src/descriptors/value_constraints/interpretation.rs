use super::{
    parse_constraint_configuration, ConfigurationDiagnosis, ConstraintConfiguration,
    ConstraintFamily, ConstraintParameters,
};
use crate::descriptors::ValidExtendsLineage;

/// Authored structural defects, distinct from operational failures acquiring schema facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintTypeDefect {
    MissingDescribedBy,
    MultipleDescribedBy(usize),
    UnusableLineage,
    NotConcreteConstraintType,
}

/// Shared interpretation, retaining authored defects without converting them into read errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintInterpretation {
    StructurallyInvalid(ConstraintTypeDefect),
    InvalidConfiguration(Vec<ConfigurationDiagnosis>),
    Parameters(ConstraintParameters),
    UnrecognizedFamily,
}

/// Interprets supplied structural evidence and owned configuration without reading schema state.
/// The caller supplies an error for a missing/multiple describer, unusable lineage, or non-concrete
/// ConstraintType. Applicability is a separate per-attachment judgment and must be checked before
/// exposing configuration or support outcomes. Successful structural evidence is shared across
/// attachments; it does not grant executable support to an extension family.
pub fn interpret_constraint(
    lineage: Result<ValidExtendsLineage<'_>, ConstraintTypeDefect>,
    configuration: &ConstraintConfiguration,
) -> ConstraintInterpretation {
    let lineage = match lineage {
        Ok(lineage) => lineage,
        Err(defect) => return ConstraintInterpretation::StructurallyInvalid(defect),
    };
    let Some(family) = ConstraintFamily::for_lineage(lineage) else {
        return ConstraintInterpretation::UnrecognizedFamily;
    };
    match parse_constraint_configuration(family, configuration) {
        Ok(parameters) => ConstraintInterpretation::Parameters(parameters),
        Err(diagnoses) => ConstraintInterpretation::InvalidConfiguration(diagnoses),
    }
}
