use core_types::{
    CommitValidationViolation, CommitValidationViolationKind, HolonError, ValidationSeverity,
    ValidationSubjectPath,
};
use holons_core::{Descriptor, ReadableHolon, ValueDescriptorKind};
use type_names::CoreValidationRuleName;

use crate::commitments::{declaring_identity, required_key};
use crate::{ResolvedValidationBinding, RuleOutcome, ValidationCollector, ValidationInvocation};

pub(crate) fn finding(
    collector: &mut ValidationCollector,
    kind: CommitValidationViolationKind,
    rule_key: Option<String>,
    path: &ValidationSubjectPath,
    descriptor_identity: Option<String>,
    message: String,
) {
    collector.record(CommitValidationViolation {
        kind,
        rule_key,
        severity: ValidationSeverity::Error,
        subject: path.clone(),
        descriptor_identity,
        message,
    });
}

pub(crate) fn rule_violation(
    collector: &mut ValidationCollector,
    binding: &ResolvedValidationBinding,
    path: &ValidationSubjectPath,
    code: &str,
    message: String,
) -> Result<(), HolonError> {
    finding(
        collector,
        CommitValidationViolationKind::RuleViolation { code: code.into() },
        Some(required_key(&binding.rule)?),
        path,
        Some(declaring_identity(binding)),
        message,
    );
    Ok(())
}

/// Requiredness applies to absent effective members, after the parent has supplied
/// the kernel's minimum decision. No containing-holon handle reaches this function.
pub(crate) fn required_property_presence(
    invocation: ValidationInvocation<'_>,
    collector: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    let (binding, path, missing_required, name, identity) = match invocation {
        ValidationInvocation::Property { binding, subject, context } => (
            binding,
            subject.path,
            subject.value.is_none()
                && context.enforce_minimum
                && subject.descriptor.is_required()?,
            subject.descriptor.property_name()?.to_string(),
            subject.descriptor.holon().reference_id_string(),
        ),
        ValidationInvocation::Prepared {
            binding,
            path,
            subject:
                crate::PreparedRuleSubject::Property { missing_required, name, descriptor_identity },
        } => (binding, path, *missing_required, name.clone(), descriptor_identity.clone()),
        _ => return Err(wrong_invocation("property")),
    };
    if missing_required {
        rule_violation(
            collector,
            binding,
            path,
            "DS-PROP-001",
            format!("Supply required property {name} (descriptor {identity})."),
        )?;
    }
    Ok(RuleOutcome::Continue)
}

/// The semantic façade owns property discovery; raw populated maps stay private.
pub(crate) fn no_undescribed_properties(
    invocation: ValidationInvocation<'_>,
    collector: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    let (binding, path, names) = match invocation {
        ValidationInvocation::Holon { binding, subject, descriptor: _, path } => {
            let names = subject.holon.undescribed_property_names()?;
            (binding, path, names)
        }
        ValidationInvocation::Prepared {
            binding,
            path,
            subject: crate::PreparedRuleSubject::Holon { undescribed_properties },
        } => (binding, path, undescribed_properties.clone()),
        _ => return Err(wrong_invocation("holon")),
    };
    for name in names {
        rule_violation(collector, binding, path, "DS-PROP-003", format!(
            "Property {name} is populated but is not described; remove it or declare it in the effective property contract."
        ))?;
    }
    Ok(RuleOutcome::Continue)
}

/// All five rules reuse the kernel's kind-only classifier. Calling `is_valid()`
/// here would execute configured semantics outside native-kind validation.
pub(crate) fn base_value_kind_matches(
    invocation: ValidationInvocation<'_>,
    collector: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    let (binding, path, expected, actual, identity) = match invocation {
        ValidationInvocation::Value { binding, subject, context } => (
            binding,
            subject.path,
            subject.descriptor.value_kind(&context.roots)?,
            subject.value.kind(),
            subject.descriptor.holon().reference_id_string(),
        ),
        ValidationInvocation::Prepared {
            binding,
            path,
            subject: crate::PreparedRuleSubject::Value { expected, actual, descriptor_identity },
        } => (binding, path, expected.clone(), *actual, descriptor_identity.clone()),
        _ => return Err(wrong_invocation("value")),
    };
    let compatible = match &expected {
        ValueDescriptorKind::BaseValue(kind) => *kind == actual,
        ValueDescriptorKind::AnyBaseValue => true,
        ValueDescriptorKind::ValueArray | ValueDescriptorKind::Unsupported(_) => false,
    };
    if !compatible {
        rule_violation(
            collector,
            binding,
            path,
            "BaseValueKindMismatch",
            format!("Expected {expected:?}, found {actual:?} for value descriptor {identity}."),
        )?;
        return Ok(RuleOutcome::StopValueEvaluation);
    }
    Ok(RuleOutcome::Continue)
}

fn wrong_invocation(expected: &str) -> HolonError {
    HolonError::InvalidParameter(format!("Validation handler requires a {expected} invocation"))
}

/// Expected native family for each fixed handler, independent of authored labels.
pub(crate) fn native_rule_kind(name: CoreValidationRuleName) -> Option<core_types::BaseValueKind> {
    use core_types::BaseValueKind;
    use CoreValidationRuleName::*;
    match name {
        BaseValueKindMatchesString => Some(BaseValueKind::String),
        BaseValueKindMatchesInteger => Some(BaseValueKind::Integer),
        BaseValueKindMatchesBoolean => Some(BaseValueKind::Boolean),
        BaseValueKindMatchesBytes => Some(BaseValueKind::Bytes),
        BaseValueKindMatchesEnum => Some(BaseValueKind::Enum),
        RequiredPropertyPresence
        | NoUndescribedProperties
        | AtMostOneDirectParent
        | AcyclicExtendsLineage
        | ExtendsLineageTerminatesAtTypeDescriptor
        | UniqueTypeDescriptorRoot
        | LocalInstanceKindAnchorDesignation
        | InstanceKindAnchorsAreAbstract
        | TypeDescriptorRootKindException
        | DescribingCategoryCompatibility
        | DescriptorMetaTypeCorrespondence
        | NoInheritedMemberRedeclaration
        | UniqueSemanticMemberNames
        | WellFormedEffectiveMemberDefinitions
        | ContractMemberKindCompatibility
        | InheritedValueConstraintNonRelaxation
        | SchemaDependenciesAcyclic
        | CrossSchemaDependenciesDeclared => None,
    }
}
