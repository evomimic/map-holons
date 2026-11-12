//! Authored-literal semantics shared by QueryCore and descriptor consumers.
use std::sync::Arc;

use super::{
    classify_constraint_type, interpret_constraint, ClassifiedConstraintType,
    ConstraintConfiguration, ConstraintEvaluationOutcome, ConstraintFamily,
    ConstraintInterpretation, ConstraintTypeDefect, ScalarConstraintFamily,
};
use crate::descriptors::{
    constraint_applies_to_lineage, Descriptor, ExtendsLineageDiagnosis, ValueDescriptor,
};
use crate::{HolonReference, ReadableHolon};
use base_types::BaseValue;
use core_types::{HolonError, SchemaInvalidityKind};
use type_names::CorePropertyTypeName;

/// Acquires schema facts through bound references; scalar evaluators see only owned parameters.
pub(in crate::descriptors) fn validate_literal(
    descriptor: &ValueDescriptor,
    value: &BaseValue,
) -> Result<(), HolonError> {
    descriptor.is_valid(value)?;
    let contributions = descriptor.effective_constraints()?;
    if contributions.is_empty() {
        return Ok(());
    }
    let descriptor_diagnosis = ExtendsLineageDiagnosis::assess(descriptor.holon())?;
    let descriptor_lineage = descriptor_diagnosis.valid_lineage().ok_or_else(|| {
        schema_invalid(
            descriptor.holon(),
            "The constrained value type has an unusable Extends lineage".into(),
        )
    })?;
    let mut prepared = Vec::new();
    for contribution in contributions {
        let constraint = &contribution.member;
        let (constraint_type, diagnosis) =
            match classify_constraint_type(constraint, |reference| {
                Ok(Arc::new(ExtendsLineageDiagnosis::assess(reference)?))
            })? {
                ClassifiedConstraintType::Concrete { reference, diagnosis } => {
                    (reference, diagnosis)
                }
                ClassifiedConstraintType::Defective { defect, describing_type } => {
                    return Err(match defect {
                        ConstraintTypeDefect::MissingDescribedBy => {
                            HolonError::MissingDescribedBy {
                                holon: constraint.reference_id_string(),
                            }
                        }
                        ConstraintTypeDefect::MultipleDescribedBy(count) => {
                            HolonError::MultipleDescribedBy {
                                holon: constraint.reference_id_string(),
                                count,
                            }
                        }
                        _ => schema_invalid(
                            descriptor.holon(),
                            format!(
                                "Constraint {} described by {} is invalid: {defect}",
                                constraint.reference_id_string(),
                                describing_type
                                    .as_ref()
                                    .map(|reference| reference.reference_id_string())
                                    .unwrap_or_default()
                            ),
                        ),
                    });
                }
            };
        let lineage = diagnosis.valid_lineage().ok_or_else(|| {
            HolonError::InvalidState(
                "Concrete constraint classification lacks a valid lineage".into(),
            )
        })?;
        if !constraint_applies_to_lineage(&constraint_type, descriptor_lineage)? {
            return Err(HolonError::DescriptorSchemaInvalid {
                kind: SchemaInvalidityKind::IncompatibleConstraintFamily,
                descriptor: descriptor.holon().reference_id_string(),
                detail: format!(
                    "Constraint {} described by {} does not apply to the value type",
                    constraint.reference_id_string(),
                    constraint_type.reference_id_string()
                ),
            });
        }
        let configuration = if ConstraintFamily::for_lineage(lineage).is_some() {
            ConstraintConfiguration::acquire(constraint)?
        } else {
            ConstraintConfiguration::default()
        };
        let parameters = match interpret_constraint(lineage, &configuration) {
            ConstraintInterpretation::Parameters(parameters) => Some(parameters),
            ConstraintInterpretation::UnrecognizedFamily => None,
            ConstraintInterpretation::InvalidConfiguration(diagnoses) => {
                return Err(schema_invalid(
                    descriptor.holon(),
                    format!(
                        "Constraint {} described by {}: {}",
                        constraint.reference_id_string(),
                        constraint_type.reference_id_string(),
                        diagnoses
                            .iter()
                            .map(|diagnosis| diagnosis.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; ")
                    ),
                ))
            }
        };
        let family = constraint_type
            .key()?
            .and_then(|key| ScalarConstraintFamily::for_concrete_type(&key.0));
        let Some(family) = family else {
            return Err(HolonError::UnsupportedValueConstraint {
                constraint_type: label(&constraint_type)?,
                value_type: label(descriptor.holon())?,
                descriptor: label(&contribution.declared_on)?,
            });
        };
        let scalar =
            parameters.and_then(|parameters| parameters.scalar_parameters()).ok_or_else(|| {
                HolonError::InvalidState(
                    "Supported scalar constraint lacks prepared scalar parameters".into(),
                )
            })?;
        if family != scalar.family() {
            return Err(HolonError::InvalidState(
                "Concrete constraint type disagrees with scalar parameters".into(),
            ));
        }
        prepared.push((contribution, constraint_type, scalar, family));
    }
    for (contribution, constraint_type, scalar, family) in prepared {
        let constraint = &contribution.member;
        match scalar.evaluate(value) {
            ConstraintEvaluationOutcome::Satisfied => {}
            ConstraintEvaluationOutcome::Violated(detail) => {
                return Err(HolonError::ValueConstraintViolation {
                    code: detail.code.into(),
                    value_type: descriptor.holon().reference_id_string(),
                    constraint: constraint.reference_id_string(),
                    constraint_type: constraint_type.reference_id_string(),
                    descriptor: contribution.declared_on.reference_id_string(),
                    detail: detail.to_string(),
                })
            }
            ConstraintEvaluationOutcome::IncompatibleValueKind => {
                return Err(HolonError::ValueKindMismatch {
                    expected: family.expected_kind().to_string(),
                    found: value.kind().to_string(),
                    descriptor: descriptor.holon().reference_id_string(),
                })
            }
        }
    }
    Ok(())
}

fn schema_invalid(descriptor: &HolonReference, detail: String) -> HolonError {
    HolonError::DescriptorSchemaInvalid {
        kind: SchemaInvalidityKind::InvalidConstraintConfiguration,
        descriptor: descriptor.reference_id_string(),
        detail,
    }
}

/// Human-readable labels supplement the identity-bearing violation contract.
fn label(reference: &HolonReference) -> Result<String, HolonError> {
    Ok(match reference.property_value(CorePropertyTypeName::TypeName)? {
        Some(BaseValue::StringValue(name)) => name.to_string(),
        _ => reference
            .key()?
            .map(|key| key.to_string())
            .unwrap_or_else(|| reference.reference_id_string()),
    })
}
