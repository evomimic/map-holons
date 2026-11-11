use core_types::HolonError;
use holons_core::ValueDescriptorKind;

use crate::diagnostics::rule_violation;
use crate::{RuleOutcome, ValidationCollector, ValidationInvocation};

/// Requiredness applies to absent effective members, after the parent has supplied
/// the kernel's minimum decision. No containing-holon handle reaches this function.
pub(crate) fn required_property_presence(
    invocation: ValidationInvocation<'_>,
    collector: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    let (binding, path, code, missing_required, name, identity) = match invocation {
        ValidationInvocation::Prepared {
            code,
            binding,
            path,
            subject:
                crate::PreparedRuleSubject::Property { missing_required, name, descriptor_identity },
        } => (binding, path, code, *missing_required, name.clone(), descriptor_identity.clone()),
        _ => return Err(wrong_invocation("property")),
    };
    if missing_required {
        rule_violation(
            collector,
            binding,
            path,
            code,
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
    let (binding, path, code, names) = match invocation {
        ValidationInvocation::Prepared {
            code,
            binding,
            path,
            subject: crate::PreparedRuleSubject::Holon { undescribed_properties, .. },
        } => (binding, path, code, undescribed_properties.clone()),
        _ => return Err(wrong_invocation("holon")),
    };
    for name in names {
        rule_violation(collector, binding, path, code, format!(
            "Property {name} is populated but is not described; remove it or declare it in the effective property contract."
        ))?;
    }
    Ok(RuleOutcome::Continue)
}

/// Contextual uniqueness consumes bounded prepared evidence, without selecting heads
/// or validating supporting saved holons. A same-key continuation has no exemption.
pub(crate) fn key_uniqueness(
    invocation: ValidationInvocation<'_>,
    collector: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    let (binding, path, code, key_claim) = match invocation {
        ValidationInvocation::Prepared {
            code,
            binding,
            path,
            subject: crate::PreparedRuleSubject::Holon { key_claim, .. },
        } => (binding, path, code, key_claim),
        _ => return Err(wrong_invocation("holon")),
    };
    if let Some(claim) = key_claim {
        let conflicts = claim.evidence.conflicting_lineages(&claim.lineage);
        if !conflicts.is_empty() {
            let conflicts =
                conflicts.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
            let message = format!(
                "Key {} for lineage {} conflicts with saved or live claims from \
                 different lineages: {conflicts}. Committed historical key associations are retained.",
                claim.evidence.key, claim.lineage,
            );
            rule_violation(collector, binding, path, code, message)?;
        }
    }
    Ok(RuleOutcome::Continue)
}

/// All five rules reuse the kernel's kind-only classifier. Calling `is_valid()`
/// here would execute configured semantics outside native-kind validation.
pub(crate) fn base_value_kind_matches(
    invocation: ValidationInvocation<'_>,
    collector: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    let (binding, path, code, expected, actual, identity) = match invocation {
        ValidationInvocation::Prepared {
            code,
            binding,
            path,
            subject: crate::PreparedRuleSubject::Value { expected, actual, descriptor_identity },
        } => (binding, path, code, expected.clone(), *actual, descriptor_identity.clone()),
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
            code,
            format!("Expected {expected:?}, found {actual:?} for value descriptor {identity}."),
        )?;
        return Ok(RuleOutcome::StopValueEvaluation);
    }
    Ok(RuleOutcome::Continue)
}

fn wrong_invocation(expected: &str) -> HolonError {
    HolonError::InvalidParameter(format!("Validation handler requires a {expected} invocation"))
}

/// Proves property binding compatibility; fixed traversal owns value findings.
pub(crate) fn property_value_conformance(
    invocation: ValidationInvocation<'_>,
    _collector: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    match invocation {
        ValidationInvocation::Prepared {
            subject: crate::PreparedRuleSubject::Property { .. },
            ..
        } => Ok(RuleOutcome::Continue),
        _ => Err(wrong_invocation("property")),
    }
}
