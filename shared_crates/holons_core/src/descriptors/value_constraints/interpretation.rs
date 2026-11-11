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

/// Structural evidence retained for interpretation without repeating lineage acquisition.
pub enum ClassifiedConstraintType {
    Defective {
        defect: ConstraintTypeDefect,
        describing_type: Option<crate::HolonReference>,
    },
    Concrete {
        reference: crate::HolonReference,
        diagnosis: std::sync::Arc<crate::ExtendsLineageDiagnosis>,
    },
}

/// Classifies authored structure while preserving backend errors and the supplied diagnosis scope.
pub fn classify_constraint_type(
    constraint: &crate::HolonReference,
    diagnose: impl FnOnce(
        &crate::HolonReference,
    ) -> Result<
        std::sync::Arc<crate::ExtendsLineageDiagnosis>,
        core_types::HolonError,
    >,
) -> Result<ClassifiedConstraintType, core_types::HolonError> {
    use crate::{DescribingTypeResolution, ReadableHolon};
    let reference = match crate::resolve_describing_type(constraint)? {
        DescribingTypeResolution::Missing => {
            return Ok(ClassifiedConstraintType::Defective {
                defect: ConstraintTypeDefect::MissingDescribedBy,
                describing_type: None,
            })
        }
        DescribingTypeResolution::Multiple(targets) => {
            return Ok(ClassifiedConstraintType::Defective {
                defect: ConstraintTypeDefect::MultipleDescribedBy(targets.len()),
                describing_type: None,
            })
        }
        DescribingTypeResolution::Unique(reference) => reference,
    };
    let diagnosis = diagnose(&reference)?;
    let defect = match diagnosis.valid_lineage() {
        None => Some(ConstraintTypeDefect::UnusableLineage),
        Some(lineage)
            if lineage.contains_role(type_names::CoreDefinitionRole::ConstraintType)
                && matches!(reference.property_value(type_names::CorePropertyTypeName::IsAbstractType)?, Some(base_types::BaseValue::BooleanValue(value)) if !value.0) =>
        {
            None
        }
        _ => Some(ConstraintTypeDefect::NotConcreteConstraintType),
    };
    Ok(match defect {
        Some(defect) => {
            ClassifiedConstraintType::Defective { defect, describing_type: Some(reference) }
        }
        None => ClassifiedConstraintType::Concrete { reference, diagnosis },
    })
}

impl std::fmt::Display for ConstraintTypeDefect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDescribedBy => {
                f.write_str("MissingDescribedBy: exactly one describing type is required")
            }
            Self::MultipleDescribedBy(count) => {
                write!(f, "MultipleDescribedBy: expected one describing type, found {count}")
            }
            Self::UnusableLineage => {
                f.write_str("the describing type has an unusable Extends lineage")
            }
            Self::NotConcreteConstraintType => f.write_str(
                "the describing type must specialize ConstraintType and have IsAbstractType false",
            ),
        }
    }
}
