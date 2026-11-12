//! Mandatory attachment discovery and scalar execution over prepared facts.
use crate::assessment::descriptor_package::{
    AttachmentApplicability, DescriptorPackages, PreparedConstraint, PreparedConstraintParameters,
};
use crate::commitments::required_key;
use crate::diagnostics::finding;
use crate::{
    ConstraintTypeKey, StaticConstraintEvaluator, StaticConstraintRegistry, SubjectLevel,
    ValidationCollector,
};
use core_types::{BaseValue, CommitValidationViolationKind, HolonError, ValidationSubjectPath};
use holons_core::descriptors::value_constraints::{
    ConstraintEvaluationOutcome, ScalarConstraintFamily, ScalarConstraintParameters,
};
use holons_core::{EffectiveRelationshipMember, HolonReference, ReadableHolon};
use type_names::CoreValidationRuleName;

/// One executable contribution retains its attachment's declaring descriptor.
pub(crate) struct PreparedValueConstraint<'p> {
    contribution: EffectiveRelationshipMember,
    constraint_type: &'p HolonReference,
    parameters: ScalarConstraintParameters,
    evaluator: StaticConstraintEvaluator,
}

/// Always discovers attachments, even when representation prevents execution.
pub(crate) fn assess_subject_constraints<'p>(
    descriptor: &HolonReference,
    level: SubjectLevel,
    path: &ValidationSubjectPath,
    collector: &mut ValidationCollector,
    packages: &'p DescriptorPackages,
) -> Result<Vec<PreparedValueConstraint<'p>>, HolonError> {
    let mut evaluators = Vec::new();
    let contributions = packages.effective_constraints(descriptor)?;
    for contribution in contributions {
        collector.observations.effective_constraint_count += 1;
        let prepared = packages.prepared_constraint(&contribution.member)?;
        let applicability = packages.attachment_applicability(descriptor, &contribution.member)?;
        let interpretation = match (prepared, applicability) {
            (PreparedConstraint::Defective { defect, .. }, _) => Err(defect.to_string()),
            (_, AttachmentApplicability::DoesNotApply) => {
                Err("Constraint is not applicable to this descriptor".into())
            }
            (_, AttachmentApplicability::Blocked(reason)) => Err(reason.clone()),
            (
                PreparedConstraint::Interpreted {
                    parameters: PreparedConstraintParameters::Invalid(diagnoses),
                    ..
                },
                _,
            ) => Err(diagnoses.iter().map(|d| d.message.as_str()).collect::<Vec<_>>().join("; ")),
            (
                PreparedConstraint::Interpreted { constraint_type, parameters },
                AttachmentApplicability::Applies,
            ) => Ok((constraint_type, parameters)),
        };
        let (constraint_type, parameters) = match interpretation {
            Ok(interpreted) => interpreted,
            Err(reason) => {
                finding(
                    collector,
                    CommitValidationViolationKind::UnresolvedLocalDependency,
                    None,
                    path,
                    Some(contribution.declared_on.reference_id_string()),
                    format!(
                        "Mandatory constraint {} is unusable: {reason}",
                        contribution.member.reference_id_string()
                    ),
                );
                continue;
            }
        };
        let key = ConstraintTypeKey(required_key(constraint_type)?);
        if let Some(evaluator) = StaticConstraintRegistry::lookup(&key, level) {
            let PreparedConstraintParameters::Valid(parameters) = parameters else {
                return Err(HolonError::InvalidState(
                    "Registered scalar constraint lacks prepared parameters".into(),
                ));
            };
            let scalar = parameters.scalar_parameters().ok_or_else(|| {
                HolonError::InvalidState(
                    "Registered scalar constraint has non-scalar parameters".into(),
                )
            })?;
            if Some(scalar.family()) != ScalarConstraintFamily::for_concrete_type(&key.0) {
                return Err(HolonError::InvalidState(
                    "Prepared scalar family disagrees with concrete constraint type".into(),
                ));
            }
            evaluators.push(PreparedValueConstraint {
                contribution,
                constraint_type,
                parameters: scalar,
                evaluator,
            });
        } else {
            finding(
                collector,
                CommitValidationViolationKind::UnsupportedConstraintType {
                    constraint_identity: contribution.member.reference_id_string(),
                    constraint_type_identity: constraint_type.reference_id_string(),
                },
                None,
                path,
                Some(contribution.declared_on.reference_id_string()),
                format!("Mandatory constraint type {} has no registered evaluator.", key.0),
            );
        }
    }
    Ok(evaluators)
}

/// Evaluates each supported contribution once, attributing findings to the supplied subject.
pub(crate) fn evaluate_value_constraints(
    value: &BaseValue,
    path: &ValidationSubjectPath,
    descriptor: &HolonReference,
    constraints: &[PreparedValueConstraint<'_>],
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    for prepared in constraints {
        collector.observations.evaluated_value_constraint_count += 1;
        let StaticConstraintEvaluator::Value(evaluate) = prepared.evaluator;
        let (kind, rule_key, message) = match evaluate(&prepared.parameters, value) {
            ConstraintEvaluationOutcome::Satisfied => continue,
            ConstraintEvaluationOutcome::Violated(detail) => (
                CommitValidationViolationKind::ConstraintViolation {
                    code: detail.code.into(),
                    constraint_identity: prepared.contribution.member.reference_id_string(),
                    constraint_type_identity: prepared.constraint_type.reference_id_string(),
                },
                Some(CoreValidationRuleName::PropertyValueConformance.as_str().into()),
                format!(
                    "Value type {}: {detail}.",
                    descriptor
                        .key()?
                        .map(|key| key.to_string())
                        .unwrap_or_else(|| descriptor.reference_id_string())
                ),
            ),
            ConstraintEvaluationOutcome::IncompatibleValueKind => (
                CommitValidationViolationKind::UnresolvedLocalDependency,
                None,
                format!(
                    "Mandatory constraint {} cannot evaluate a value of kind {}.",
                    prepared.contribution.member.reference_id_string(),
                    value.kind()
                ),
            ),
        };
        finding(
            collector,
            kind,
            rule_key,
            path,
            Some(prepared.contribution.declared_on.reference_id_string()),
            message,
        );
    }
    Ok(())
}
