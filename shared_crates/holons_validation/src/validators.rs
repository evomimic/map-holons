use core_types::{CommitValidationViolationKind, HolonError, ValidationSubjectPath};
use holons_core::{
    equals_or_extends, Descriptor, HolonDescriptor, HolonReference, ReadableHolon, ValueDescriptor,
    ValueDescriptorKind,
};

use crate::commitments::{declaring_identity, required_key};
use crate::contexts::{BindingRoots, SubjectLevel};
use crate::handlers::{finding, native_rule_kind, rule_violation};
use crate::*;

/// Assesses a complete effective holon contract without mutating the subject.
///
/// Missing and ambiguous descriptors produce distinct `NoDescriptor` diagnostics
/// and stop descriptor-dependent work for this holon. Other runtime failures return
/// `Err`; the caller must discard the incomplete pass rather than accept its report.
pub fn validate_holon(
    subject: HolonValidationSubject<'_>,
    context: &HolonValidationContext,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let identity = subject.holon.reference_id_string();
    let path = ValidationSubjectPath::Holon { holon_identity: identity.clone() };
    let descriptor = match subject.holon.holon_descriptor() {
        Ok(descriptor) => descriptor,
        Err(
            error
            @ (HolonError::MissingDescribedBy { .. } | HolonError::MultipleDescribedBy { .. }),
        ) => {
            // Stable error variant names keep the two bootstrap failures distinguishable
            // without changing the already-published dependency-safe finding enum.
            let reason = if matches!(error, HolonError::MissingDescribedBy { .. }) {
                "MissingDescribedBy"
            } else {
                "MultipleDescribedBy"
            };
            finding(
                collector,
                CommitValidationViolationKind::NoDescriptor,
                None,
                &path,
                None,
                format!("{reason}: {error}"),
            );
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let bindings =
        prepare_bindings(&descriptor, SubjectLevel::Holon, &context.values, &path, collector)?;
    for (binding, handler) in &bindings {
        mark_dispatched(binding, collector)?;
        handler(
            ValidationInvocation::Holon { binding, subject, descriptor: &descriptor, path: &path },
            collector,
        )?;
    }
    assess_constraints(&descriptor, &path, collector)?;

    // Enumerate the descriptor contract, not the populated map: absence is a subject.
    for property in descriptor.instance_properties()? {
        let name = property.property_name()?;
        let value = subject.holon.property_value(&name)?;
        let property_path = ValidationSubjectPath::Property {
            holon_identity: identity.clone(),
            name: name.to_string(),
        };
        let property_context = PropertyValidationContext {
            enforce_minimum: context.universal.enforce_minimum(subject.holon, property.holon())?,
            values: &context.values,
        };
        validate_property(
            PropertyValidationSubject {
                descriptor: &property,
                value: value.as_ref(),
                path: &property_path,
            },
            &property_context,
            collector,
        )?;
    }
    Ok(())
}

/// Assesses one effective property, then descends only when its value is populated.
/// A non-Property diagnostic path is a caller error, rejected before any assessment.
pub fn validate_property(
    subject: PropertyValidationSubject<'_>,
    context: &PropertyValidationContext<'_>,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let ValidationSubjectPath::Property { holon_identity, name } = subject.path else {
        return Err(HolonError::InvalidParameter(
            "Property validation requires a Property diagnostic path".into(),
        ));
    };
    let descriptor = HolonDescriptor::from_holon(subject.descriptor.holon().clone());
    let bindings = prepare_bindings(
        &descriptor,
        SubjectLevel::Property,
        context.values,
        subject.path,
        collector,
    )?;
    for (binding, handler) in &bindings {
        mark_dispatched(binding, collector)?;
        handler(ValidationInvocation::Property { binding, subject, context }, collector)?;
    }
    assess_constraints(&descriptor, subject.path, collector)?;
    if let Some(value) = subject.value {
        let value_descriptor = subject.descriptor.value_type()?;
        // Provenance changes shape, but cannot be used to navigate to the parent.
        let path = ValidationSubjectPath::Value {
            holon_identity: holon_identity.clone(),
            property: name.clone(),
        };
        validate_value(
            ValueValidationSubject { descriptor: &value_descriptor, value, path: &path },
            context.values,
            collector,
        )?;
    }
    Ok(())
}

/// Assesses fixed native-kind commitments before type-specific value evaluation.
pub fn validate_value(
    subject: ValueValidationSubject<'_>,
    context: &ValueValidationContext,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let descriptor = HolonDescriptor::from_holon(subject.descriptor.holon().clone());
    let bindings =
        prepare_bindings(&descriptor, SubjectLevel::Value, context, subject.path, collector)?;
    // Unsupported commitments are discovered before a native mismatch can stop
    // evaluation. Discovery is not type-specific constraint execution.
    assess_constraints(&descriptor, subject.path, collector)?;
    for (binding, handler) in &bindings {
        mark_dispatched(binding, collector)?;
        if handler(ValidationInvocation::Value { binding, subject, context }, collector)?
            == RuleOutcome::StopValueEvaluation
        {
            break;
        }
    }
    Ok(())
}

/// Discover and check every effective binding before invoking any handler.
/// Binding compatibility is a schema-conformance finding, not a dispatch fallback.
fn prepare_bindings(
    descriptor: &HolonDescriptor,
    level: SubjectLevel,
    context: &ValueValidationContext,
    path: &ValidationSubjectPath,
    collector: &mut ValidationCollector,
) -> Result<Vec<(ResolvedValidationBinding, StaticRuleHandler)>, HolonError> {
    let mut prepared = Vec::new();
    for contribution in descriptor.effective_validation_bindings()? {
        let binding = ResolvedValidationBinding::from(contribution);
        let key = ValidationRuleKey(required_key(&binding.rule)?);
        collector.observations.discovered_rule_keys.insert(key.0.clone());
        if !compatible_binding(&binding, descriptor.holon(), level, &context.bindings, context)? {
            rule_violation(collector, &binding, path, "IncompatibleValidationBinding",
                "The rule family, declaring descriptor, and subject kind must be compatible before dispatch.".into())?;
            continue;
        }
        match StaticRuleRegistry::lookup(&key) {
            Some(handler) => prepared.push((binding, handler)),
            None => finding(
                collector,
                CommitValidationViolationKind::UnsupportedValidationRule,
                Some(key.0),
                path,
                Some(declaring_identity(&binding)),
                "Mandatory validation binding has no registered handler.".into(),
            ),
        }
    }
    Ok(prepared)
}

fn compatible_binding(
    binding: &ResolvedValidationBinding,
    governing: &HolonReference,
    level: SubjectLevel,
    roots: &BindingRoots,
    context: &ValueValidationContext,
) -> Result<bool, HolonError> {
    let family = binding.rule.holon_descriptor()?;
    let key = required_key(&binding.rule)?;
    if let Some(root) = roots.entries.iter().find(|entry| entry.name.as_str() == key) {
        // A familiar key on another reference cannot impersonate a canonical rule.
        if binding.rule.reference_id_string() != root.rule.reference_id_string()
            || root.level != level
            || !equals_or_extends(family.holon(), &root.family)?
            || !equals_or_extends(binding.declaring_descriptor.holon(), &root.descriptor_family)?
        {
            return Ok(false);
        }
        if let Some(expected) = native_rule_kind(root.name) {
            return Ok(ValueDescriptor::from_holon(governing.clone())
                .value_kind(&context.roots)?
                == ValueDescriptorKind::BaseValue(expected));
        }
        return Ok(true);
    }
    // Unknown commitments still fail closed. Where C1 knows their family, reject
    // malformed placement explicitly before considering handler availability.
    for root in &roots.entries {
        if equals_or_extends(family.holon(), &root.family)? {
            return Ok(root.level == level
                && equals_or_extends(
                    binding.declaring_descriptor.holon(),
                    &root.descriptor_family,
                )?);
        }
    }
    Ok(true)
}

fn mark_dispatched(
    binding: &ResolvedValidationBinding,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    collector.observations.dispatched_rule_keys.insert(required_key(&binding.rule)?);
    Ok(())
}

/// C1 has no configured evaluators. Reaching a constraint is a blocking finding,
/// independent of whether its type is one the future capability will support.
fn assess_constraints(
    descriptor: &HolonDescriptor,
    path: &ValidationSubjectPath,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    for contribution in descriptor.effective_constraints()? {
        collector.observations.effective_constraint_count += 1;
        let constraint = ResolvedConstraint::try_from(contribution)?;
        let key = ConstraintTypeKey(required_key(constraint.constraint_type.holon())?);
        if StaticConstraintRegistry::lookup(&key).is_none() {
            let constraint_identity = constraint.constraint.reference_id_string();
            let constraint_type_identity = constraint.constraint_type.holon().reference_id_string();
            finding(
                collector,
                CommitValidationViolationKind::UnsupportedConstraintType {
                    constraint_identity,
                    constraint_type_identity,
                },
                None,
                path,
                Some(constraint.declaring_descriptor.holon().reference_id_string()),
                format!("Mandatory constraint type {} has no registered evaluator.", key.0),
            );
        }
    }
    Ok(())
}
