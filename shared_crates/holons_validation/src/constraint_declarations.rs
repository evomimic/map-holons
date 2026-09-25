//! Constraint declaration assessment. This module never calls a subject constraint evaluator.
//! DS-CONSTRAINT-001 prepares a seeded rule product; -002 and -003 are fixed checks.
//! Instantiate one assessment per prospective Schema, then discard it before mutation.

use std::collections::HashSet;

use base_types::BaseValue;
use core_types::{CommitValidationViolationKind, PropertyName, ValidationSubjectPath};
use holons_core::{
    descriptors::{
        constraint_applies_to_lineage_with_reader, constraint_applies_to_with_reader,
        effective_relationship_targets_with_reader, resolve_describing_type_with_reader,
        same_definition, ConstraintContributions, DescribingTypeResolution,
        ExtendsLineageDiagnosis, ValidExtendsLineage,
    },
    DescriptorReader, HolonReference, ReadableHolon,
};
use type_names::{
    CorePropertyTypeName as P, CoreRelationshipTypeName as R, CoreValidationRuleName,
};

use crate::{handlers::finding, DescriptorRuleProducts, ValidationCollector};

/// Canonical identities resolved by orchestration, using its prospective reader.
/// Roots are resolved for the Commit assessment scope.
pub struct ConstraintDeclarationRoots {
    /// Designated TypeDescriptor root used for structural diagnosis.
    pub type_descriptor: HolonReference,
    /// ConstraintType.HolonType; extension constraint types must specialize it.
    pub constraint_type: HolonReference,
    /// StringLengthConstraint, BytesLengthConstraint, NumericRangeConstraint,
    /// and ItemCountConstraint, in any order. All share the normalized bound contract.
    pub bounded: [HolonReference; 4],
    /// CardinalityConstraint.ConstraintType, with inclusive integer endpoints.
    pub cardinality: HolonReference,
    /// UniqueItemsConstraint.ConstraintType, with no enable/disable parameter.
    pub unique_items: HolonReference,
}

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
pub struct ConstraintDeclarationAssessment<'a, Reader: DescriptorReader> {
    roots: ConstraintDeclarationRoots,
    reader: &'a Reader,
    checked: Vec<CheckedConstraint>,
}

impl<'a, Reader: DescriptorReader> ConstraintDeclarationAssessment<'a, Reader> {
    /// Creates an empty Schema-scoped declaration workset over an unchanged input snapshot.
    pub fn new(
        roots: &ConstraintDeclarationRoots,
        reader: &'a Reader,
    ) -> Result<Self, Reader::Error> {
        // Select fixed roots once, before any findings or memo entries are produced.
        let roots = ConstraintDeclarationRoots {
            type_descriptor: reader.select(&roots.type_descriptor)?,
            constraint_type: reader.select(&roots.constraint_type)?,
            bounded: [
                reader.select(&roots.bounded[0])?,
                reader.select(&roots.bounded[1])?,
                reader.select(&roots.bounded[2])?,
                reader.select(&roots.bounded[3])?,
            ],
            cardinality: reader.select(&roots.cardinality)?,
            unique_items: reader.select(&roots.unique_items)?,
        };
        Ok(Self { roots, reader, checked: Vec::new() })
    }

    /// Checks a configured holon once, including owned constraints with no attachments.
    /// Unknown extension families require no evaluator just to declare their configuration.
    pub fn assess_constraint(
        &mut self,
        constraint: &HolonReference,
        collector: &mut ValidationCollector,
    ) -> Result<bool, Reader::Error> {
        Ok(self.check_constraint(constraint, collector)?.valid)
    }

    /// Reads kernel contributions only after structural prerequisites succeeded.
    pub fn assess_descriptor(
        &mut self,
        lineage: ValidExtendsLineage<'_>,
        products: &mut DescriptorRuleProducts,
        collector: &mut ValidationCollector,
    ) -> Result<bool, Reader::Error> {
        let contributions = ConstraintContributions::resolve_with_reader(lineage, self.reader)?;
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
    ) -> Result<bool, Reader::Error> {
        let subject = self.reader.select(lineage.subject())?;
        let mut valid = true;
        for attachment in &contributions.effective {
            collector.observations.constraint_attachment_count += 1;
            let checked = self.check_constraint(&attachment.member, collector)?;
            if let Some(constraint_type) = &checked.constraint_type {
                if !constraint_applies_to_lineage_with_reader(
                    constraint_type,
                    lineage,
                    self.reader,
                )? {
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
            for inherited in contributions.missing_inherited() {
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
                if constraint_applies_to_with_reader(constraint_type, parent, self.reader)? {
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
    ) -> Result<CheckedConstraint, Reader::Error> {
        let constraint = self.reader.select(constraint)?;
        if let Some(checked) =
            self.checked.iter().find(|checked| same_definition(&checked.constraint, &constraint))
        {
            return Ok(checked.clone());
        }
        collector.observations.constraint_declaration_count += 1;
        let resolution = resolve_describing_type_with_reader(&constraint, self.reader)?;
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
            let diagnosis = ExtendsLineageDiagnosis::assess_with_reader(
                &constraint_type,
                &self.roots.type_descriptor,
                self.reader,
            )?;
            if let Some(lineage) = diagnosis.valid_lineage() {
                let root = &self.roots.constraint_type;
                if lineage.members().iter().any(|member| same_definition(member, root))
                    && matches!(constraint_type.property_value(P::IsAbstractType)?, Some(BaseValue::BooleanValue(value)) if !value.0)
                {
                    let configuration = Configuration {
                        constraint: &constraint,
                        constraint_type: &constraint_type,
                    };
                    let membership_valid =
                        configuration.check_membership(self.reader, collector)?;
                    let family_valid = self.check_family(lineage, &configuration, collector)?;
                    checked.valid = membership_valid && family_valid;
                } else {
                    fixed_violation(collector, &constraint, "DS-CONSTRAINT-003", format!(
                        "Constraint {} must select a concrete ConstraintType; describing type {} must specialize {} and have IsAbstractType false.",
                        constraint.reference_id_string(), constraint_type.reference_id_string(), root.reference_id_string()));
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
    ) -> Result<bool, Reader::Error> {
        let mut valid = true;
        // Family checks are identity-based and also apply to extension-owned subtypes.
        for root in &self.roots.bounded {
            if lineage.members().iter().any(|member| same_definition(member, root)) {
                valid &= configuration.check_bounds(false, collector)?;
                break;
            }
        }
        let cardinality = &self.roots.cardinality;
        if lineage.members().iter().any(|member| same_definition(member, cardinality)) {
            valid &= configuration.check_bounds(true, collector)?;
        }
        let unique = &self.roots.unique_items;
        if lineage.members().iter().any(|member| same_definition(member, unique)) {
            // No Boolean parameter controls this commitment. Undeclared parameters,
            // including any enable/disable field, are rejected by membership checking.
            for field in [P::Minimum, P::Maximum, P::MinimumIsInclusive, P::MaximumIsInclusive] {
                if configuration.constraint.property_value(&field)?.is_some() {
                    configuration.violation(
                        collector,
                        format!(
                            "UniqueItemsConstraint is presence-only and has no {} parameter.",
                            field.as_property_name()
                        ),
                    );
                    valid = false;
                }
            }
        }
        Ok(valid)
    }
}

struct Configuration<'a> {
    constraint: &'a HolonReference,
    constraint_type: &'a HolonReference,
}

/// Retains presence separately from native-kind validity for conditional fields.
struct Field<T> {
    present: bool,
    value: Option<T>,
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

    fn check_membership<Reader: DescriptorReader>(
        &self,
        reader: &Reader,
        collector: &mut ValidationCollector,
    ) -> Result<bool, Reader::Error> {
        let mut names = HashSet::new();
        for contribution in effective_relationship_targets_with_reader(
            self.constraint_type,
            R::InstanceProperties,
            reader,
        )? {
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

    fn integer(
        &self,
        field: P,
        collector: &mut ValidationCollector,
    ) -> Result<Field<i64>, core_types::HolonError> {
        let value = self.constraint.property_value(&field)?;
        let parsed = match &value {
            Some(BaseValue::IntegerValue(value)) if value.0 >= 0 => Some(value.0),
            Some(BaseValue::IntegerValue(value)) => {
                self.violation(
                    collector,
                    format!(
                        "{} must be non-negative; found integer {}.",
                        field.as_property_name(),
                        value.0
                    ),
                );
                None
            }
            None => None,
            Some(value) => {
                self.violation(
                    collector,
                    format!(
                        "{} must be an integer; found value kind {:?}.",
                        field.as_property_name(),
                        value.kind()
                    ),
                );
                None
            }
        };
        Ok(Field { present: value.is_some(), value: parsed })
    }

    fn boolean(
        &self,
        field: P,
        collector: &mut ValidationCollector,
    ) -> Result<Field<bool>, core_types::HolonError> {
        let value = self.constraint.property_value(&field)?;
        let parsed = match &value {
            Some(BaseValue::BooleanValue(value)) => Some(value.0),
            None => None,
            _ => {
                self.violation(collector, format!("{} must be Boolean.", field.as_property_name()));
                None
            }
        };
        Ok(Field { present: value.is_some(), value: parsed })
    }

    fn check_bounds(
        &self,
        cardinality: bool,
        collector: &mut ValidationCollector,
    ) -> Result<bool, core_types::HolonError> {
        let minimum = self.integer(P::Minimum, collector)?;
        let maximum = self.integer(P::Maximum, collector)?;
        let lower_inclusive = self.boolean(P::MinimumIsInclusive, collector)?;
        let upper_inclusive = self.boolean(P::MaximumIsInclusive, collector)?;
        let mut valid = (!minimum.present || minimum.value.is_some())
            && (!maximum.present || maximum.value.is_some())
            && (!lower_inclusive.present || lower_inclusive.value.is_some())
            && (!upper_inclusive.present || upper_inclusive.value.is_some());
        if cardinality {
            if !minimum.present {
                self.violation(collector, "CardinalityConstraint requires Minimum.".into());
                valid = false;
            }
            if lower_inclusive.present || upper_inclusive.present {
                self.violation(
                    collector,
                    "CardinalityConstraint has inclusive bounds and admits no inclusivity flags."
                        .into(),
                );
                valid = false;
            }
        } else {
            if !minimum.present && !maximum.present {
                self.violation(
                    collector,
                    "At least one of Minimum and Maximum is required.".into(),
                );
                valid = false;
            }
            for (bound, flag, name) in [
                (minimum.present, lower_inclusive.present, "MinimumIsInclusive"),
                (maximum.present, upper_inclusive.present, "MaximumIsInclusive"),
            ] {
                if bound != flag {
                    self.violation(
                        collector,
                        format!("{name} must be present exactly when its bound is present."),
                    );
                    valid = false;
                }
            }
        }
        // Missing or malformed flags already have findings. Equal-endpoint checks
        // require two valid flags so their absence does not produce a second interval finding.
        if let (Some(minimum), Some(maximum)) = (minimum.value, maximum.value) {
            if minimum > maximum
                || (minimum == maximum
                    && !cardinality
                    && matches!((lower_inclusive.value, upper_inclusive.value), (Some(lower), Some(upper)) if !lower || !upper))
            {
                self.violation(
                    collector,
                    "Bounds form an invalid interval; equal endpoints require both ends inclusive."
                        .into(),
                );
                valid = false;
            }
        }
        Ok(valid)
    }
}

fn path(subject: &HolonReference) -> ValidationSubjectPath {
    ValidationSubjectPath::Holon { holon_identity: subject.reference_id_string() }
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
