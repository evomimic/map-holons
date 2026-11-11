//! Constraint declaration assessment. This module never calls a subject constraint evaluator.
//! DS-CONSTRAINT-001 prepares a seeded rule product; -002 and -003 are fixed checks.
//! Instantiate one assessment per prospective Schema, then discard it before mutation.

use std::collections::HashSet;

use base_types::BaseValue;
use core_types::{CommitValidationViolationKind, HolonError, PropertyName};
use holons_core::{
    descriptors::{
        constraint_applies_to, constraint_applies_to_lineage, resolve_describing_type,
        same_definition,
        value_constraints::{
            parse_constraint_configuration, ConstraintConfiguration, ConstraintFamily,
        },
        ConstraintContributions, DescribingTypeResolution, ValidExtendsLineage,
    },
    HolonReference, ReadableHolon,
};
use type_names::{
    CoreDefinitionRole, CorePropertyTypeName as P, CoreRelationshipTypeName as R,
    CoreValidationRuleName,
};

use crate::{
    assessment::support::path, diagnostics::finding, DescriptorRuleProducts, ValidationCollector,
};

#[derive(Clone)]
struct CheckedConstraint {
    constraint: HolonReference,
    constraint_type: Option<HolonReference>,
    valid: bool,
}

/// Declaration workset for one Schema assessment, including dependency-owned reusable rules.
/// Calls return false for diagnosed invalid declarations and Err for unreadable state.
/// An Err invalidates the entire pass: discard its collector, products, and this assessment.
/// Reuse one collector for the scope; a fresh collector requires a fresh assessment,
/// because repeated declarations reuse their first result without emitting it again.
/// This collector spans subjects: outcome installation must route each finding by its
/// structured subject, never install the whole scope on its first candidate.
///
/// Own-contract subject validation of constraint holons remains a separate orchestration step.
/// This pass adds strict parameter membership and family configuration invariants; it
/// neither evaluates attached constraints nor replaces the shared subject validators.
pub struct ConstraintDeclarationAssessment {
    checked: Vec<CheckedConstraint>,
    diagnoses: std::sync::Arc<crate::assessment::structural_diagnoses::StructuralDiagnoses>,
}

impl ConstraintDeclarationAssessment {
    pub(crate) fn new(
        diagnoses: std::sync::Arc<crate::assessment::structural_diagnoses::StructuralDiagnoses>,
    ) -> Self {
        Self { diagnoses, checked: Vec::new() }
    }

    /// Checks a configured holon once, including owned constraints with no attachments.
    /// Unknown extension families require no evaluator just to declare their configuration.
    pub fn assess_constraint(
        &mut self,
        constraint: &HolonReference,
        collector: &mut ValidationCollector,
    ) -> Result<bool, HolonError> {
        Ok(self.check_constraint(constraint, collector)?.valid)
    }

    /// Reads kernel contributions only after structural prerequisites succeeded.
    pub fn assess_descriptor(
        &mut self,
        lineage: ValidExtendsLineage<'_>,
        products: &mut DescriptorRuleProducts,
        collector: &mut ValidationCollector,
    ) -> Result<bool, HolonError> {
        let contributions = ConstraintContributions::resolve(lineage)?;
        self.assess_contributions(lineage, &contributions, products, collector)
    }

    /// Assesses a kernel-produced snapshot without rebuilding or normalizing its contributions.
    /// The supplied product must describe this lineage in the same prospective snapshot.
    pub fn assess_contributions(
        &mut self,
        lineage: ValidExtendsLineage<'_>,
        contributions: &ConstraintContributions,
        products: &mut DescriptorRuleProducts,
        collector: &mut ValidationCollector,
    ) -> Result<bool, HolonError> {
        let subject = (lineage.subject()).clone();
        let mut valid = true;
        for attachment in &contributions.effective {
            collector.observations.constraint_attachment_count += 1;
            let checked = self.check_constraint(&attachment.member, collector)?;
            if let Some(constraint_type) = &checked.constraint_type {
                if !constraint_applies_to_lineage(constraint_type, lineage)? {
                    fixed_violation(collector, &subject, "DS-CONSTRAINT-002", format!(
                        "Constraint {} of type {} contributed by {} is not applicable to descriptor {} through ApplicableToDescriptorTypes and Extends.",
                        checked.constraint.reference_id_string(), constraint_type.reference_id_string(),
                        attachment.declared_on.reference_id_string(), subject.reference_id_string()));
                    valid = false;
                }
            }
            if !checked.valid {
                dependency(
                    collector,
                    &subject,
                    format!(
                        "Descriptor {} depends on invalid constraint {} contributed by {}.",
                        subject.reference_id_string(),
                        checked.constraint.reference_id_string(),
                        attachment.declared_on.reference_id_string()
                    ),
                );
                valid = false;
            }
        }
        if let Some(parent) = &contributions.parent {
            for inherited in contributions.missing_inherited()? {
                // Even an obligation removed by malformed effective-state construction must
                // be inspected; otherwise loss could suppress its own preservation check.
                let checked = self.check_constraint(&inherited.member, collector)?;
                valid &= checked.valid;
                let rule = CoreValidationRuleName::InheritedValueConstraintNonRelaxation;
                let Some(constraint_type) = &checked.constraint_type else {
                    products.unresolved(rule, format!(
                        "Cannot assess inherited obligation {} from {} until its constraint type is corrected.",
                        inherited.member.reference_id_string(), parent.reference_id_string()));
                    valid = false;
                    continue;
                };
                if constraint_applies_to(constraint_type, parent)? {
                    products.violation(rule, format!(
                        "Descriptor {} loses inherited constraint {} from parent {} with original contribution on {}; retain the obligation and its provenance in the combined effective set.",
                        subject.reference_id_string(), inherited.member.reference_id_string(),
                        parent.reference_id_string(), inherited.declared_on.reference_id_string()));
                    valid = false;
                }
            }
        }
        Ok(valid)
    }

    fn check_constraint(
        &mut self,
        constraint: &HolonReference,
        collector: &mut ValidationCollector,
    ) -> Result<CheckedConstraint, HolonError> {
        let constraint = constraint.clone();
        for checked in &self.checked {
            if same_definition(&checked.constraint, &constraint)? {
                return Ok(checked.clone());
            }
        }
        collector.observations.constraint_declaration_count += 1;
        let resolution = resolve_describing_type(&constraint)?;
        let constraint_type = match resolution {
            DescribingTypeResolution::Unique(descriptor) => Some(descriptor),
            DescribingTypeResolution::Missing => {
                finding(
                    collector,
                    CommitValidationViolationKind::NoDescriptor,
                    None,
                    &path(&constraint),
                    None,
                    format!("Constraint {}: MissingDescribedBy.", constraint.reference_id_string()),
                );
                None
            }
            DescribingTypeResolution::Multiple(targets) => {
                finding(
                    collector,
                    CommitValidationViolationKind::NoDescriptor,
                    None,
                    &path(&constraint),
                    None,
                    format!(
                        "Constraint {}: MultipleDescribedBy ({} targets).",
                        constraint.reference_id_string(),
                        targets.len()
                    ),
                );
                None
            }
        };
        let mut checked =
            CheckedConstraint { constraint: constraint.clone(), constraint_type, valid: false };
        if let Some(constraint_type) = checked.constraint_type.clone() {
            let diagnosis = self.diagnoses.get(&constraint_type)?;
            if let Some(lineage) = diagnosis.valid_lineage() {
                let root = CoreDefinitionRole::ConstraintType;
                if lineage.contains_role(root)
                    && matches!(constraint_type.property_value(P::IsAbstractType)?, Some(BaseValue::BooleanValue(value)) if !value.0)
                {
                    let configuration = Configuration {
                        constraint: &constraint,
                        constraint_type: &constraint_type,
                    };
                    let membership_valid = configuration.check_membership(collector, lineage)?;
                    let family_valid = self.check_family(lineage, &configuration, collector)?;
                    checked.valid = membership_valid && family_valid;
                } else {
                    fixed_violation(collector, &constraint, "DS-CONSTRAINT-003", format!(
                        "Constraint {} must select a concrete ConstraintType; describing type {} must specialize {} and have IsAbstractType false.",
                        constraint.reference_id_string(), constraint_type.reference_id_string(), root.key()));
                    checked.constraint_type = None;
                }
            } else {
                dependency(collector, &constraint, format!(
                    "Constraint {} cannot use describing type {} until its Extends lineage is corrected.",
                    constraint.reference_id_string(), constraint_type.reference_id_string()));
                checked.constraint_type = None;
            }
        }
        self.checked.push(checked.clone());
        Ok(checked)
    }

    fn check_family(
        &self,
        lineage: ValidExtendsLineage<'_>,
        configuration: &Configuration<'_>,
        collector: &mut ValidationCollector,
    ) -> Result<bool, HolonError> {
        let Some(family) = ConstraintFamily::for_lineage(lineage) else {
            return Ok(true);
        };
        let authored = ConstraintConfiguration::acquire(configuration.constraint)?;
        match parse_constraint_configuration(family, &authored) {
            Ok(_) => Ok(true),
            Err(diagnoses) => {
                for diagnosis in diagnoses {
                    configuration.violation(collector, diagnosis.message);
                }
                Ok(false)
            }
        }
    }
}

struct Configuration<'a> {
    constraint: &'a HolonReference,
    constraint_type: &'a HolonReference,
}

impl Configuration<'_> {
    fn violation(&self, collector: &mut ValidationCollector, message: String) {
        fixed_violation(
            collector,
            self.constraint,
            "DS-CONSTRAINT-003",
            format!(
                "Constraint {} described by {}: {message}",
                self.constraint.reference_id_string(),
                self.constraint_type.reference_id_string()
            ),
        );
    }

    fn check_membership(
        &self,
        collector: &mut ValidationCollector,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<bool, HolonError> {
        let mut names = HashSet::new();
        for contribution in lineage.relationship_targets(R::InstanceProperties)? {
            match contribution.member.property_value(P::TypeName)? {
                Some(BaseValue::StringValue(name)) if !name.0.is_empty() => {
                    names.insert(PropertyName(name));
                }
                _ => {
                    dependency(collector, self.constraint, format!(
                        "Constraint {} cannot resolve parameter membership: property {} contributed by {} lacks a non-empty string TypeName.",
                        self.constraint.reference_id_string(), contribution.member.reference_id_string(),
                        contribution.declared_on.reference_id_string()));
                    return Ok(false);
                }
            }
        }
        let undescribed = self.constraint.undescribed_property_names_in_contract(&names)?;
        for name in &undescribed {
            self.violation(
                collector,
                format!("Parameter {name} is not declared by the effective constraint type."),
            );
        }
        Ok(undescribed.is_empty())
    }
}

fn fixed_violation(
    collector: &mut ValidationCollector,
    subject: &HolonReference,
    code: &str,
    message: String,
) {
    finding(
        collector,
        CommitValidationViolationKind::RuleViolation { code: code.into() },
        None,
        &path(subject),
        None,
        message,
    );
}

fn dependency(collector: &mut ValidationCollector, subject: &HolonReference, message: String) {
    finding(
        collector,
        CommitValidationViolationKind::UnresolvedLocalDependency,
        None,
        &path(subject),
        None,
        message,
    );
}
