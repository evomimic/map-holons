//! Transient response projection for findings without a staged holon carrier.

use std::sync::Arc;

use base_types::MapString;
use core_types::{
    CommitValidationViolation, CommitValidationViolationKind, HolonError, ValidationSubjectPath,
};
use holons_core::{
    core_shared_objects::transactions::TransactionContext, descriptors::resolve_core_descriptor,
    HolonReference, WritableHolon,
};

/// Project each unattached finding into a transient holon. Descriptor resolution is
/// best effort while bootstrap has not yet installed the carrier descriptor; creating
/// or populating a carrier is required and any failure remains operational.
pub(super) fn make_finding_holons_best_effort<'a>(
    context: &Arc<TransactionContext>,
    findings: impl Iterator<Item = &'a CommitValidationViolation>,
) -> Result<Vec<HolonReference>, HolonError> {
    // Avoid descriptor lookup while no response carrier is needed.
    let findings: Vec<_> = findings.collect();
    if findings.is_empty() {
        return Ok(Vec::new());
    }

    let descriptor = resolve_core_descriptor(context, "CommitValidationFinding.Projection").ok();
    let mut carriers = Vec::with_capacity(findings.len());
    for finding in findings {
        let mut carrier = context.mutation().new_holon(None)?;
        if let Some(descriptor) = &descriptor {
            carrier.with_descriptor(descriptor.clone())?;
        }
        for (name, value) in finding_fields(finding) {
            carrier.with_property_value(name, MapString(value))?;
        }
        carriers.push(HolonReference::Transient(carrier));
    }
    Ok(carriers)
}

fn finding_fields(finding: &CommitValidationViolation) -> Vec<(&'static str, String)> {
    let mut fields = Vec::with_capacity(12);
    let kind = match &finding.kind {
        CommitValidationViolationKind::NoDescriptor => "NoDescriptor",
        CommitValidationViolationKind::UnsupportedValidationRule => "UnsupportedValidationRule",
        CommitValidationViolationKind::UnsupportedConstraintType {
            constraint_identity,
            constraint_type_identity,
        } => {
            fields.push(("ConstraintIdentity", constraint_identity.clone()));
            fields.push(("ConstraintTypeIdentity", constraint_type_identity.clone()));
            "UnsupportedConstraintType"
        }
        CommitValidationViolationKind::RuleViolation { code } => {
            fields.push(("RuleCode", code.clone()));
            "RuleViolation"
        }
        CommitValidationViolationKind::UnresolvedLocalDependency => "UnresolvedLocalDependency",
        CommitValidationViolationKind::RelationshipCoordinationRequired => {
            "RelationshipCoordinationRequired"
        }
    };
    fields.push(("ViolationKind", kind.into()));
    if let Some(rule_key) = &finding.rule_key {
        fields.push(("RuleIdentity", rule_key.clone()));
    }
    let severity = match finding.severity {
        core_types::ValidationSeverity::Info => "Info",
        core_types::ValidationSeverity::Warning => "Warning",
        core_types::ValidationSeverity::Error => "Error",
    };
    fields.push(("Severity", severity.into()));
    let (subject_kind, holon_identity, member_name, target_identity) = match &finding.subject {
        ValidationSubjectPath::Holon { holon_identity } => {
            ("Holon", Some(holon_identity), None, None)
        }
        ValidationSubjectPath::Property { holon_identity, name } => {
            ("Property", Some(holon_identity), Some(name), None)
        }
        ValidationSubjectPath::Value { holon_identity, property } => {
            ("Value", Some(holon_identity), Some(property), None)
        }
        ValidationSubjectPath::Relationship { source_identity, name, target_identity } => {
            ("Relationship", Some(source_identity), Some(name), Some(target_identity))
        }
        ValidationSubjectPath::Transaction => ("Transaction", None, None, None),
    };
    fields.push(("SubjectKind", subject_kind.into()));
    if let Some(identity) = holon_identity {
        fields.push(("HolonIdentity", identity.clone()));
    }
    if let Some(name) = member_name {
        fields.push(("MemberName", name.clone()));
    }
    if let Some(identity) = target_identity {
        fields.push(("TargetIdentity", identity.clone()));
    }
    fields.push(("Message", finding.message.clone()));
    if let Some(identity) = &finding.descriptor_identity {
        fields.push(("DescriptorIdentity", identity.clone()));
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::ValidationSeverity;

    #[test]
    fn projection_preserves_all_existing_finding_fields() {
        let finding = CommitValidationViolation {
            kind: CommitValidationViolationKind::UnsupportedConstraintType {
                constraint_identity: "constraint".into(),
                constraint_type_identity: "type".into(),
            },
            rule_key: Some("rule".into()),
            severity: ValidationSeverity::Warning,
            subject: ValidationSubjectPath::Relationship {
                source_identity: "source".into(),
                name: "LinkedTo".into(),
                target_identity: "target".into(),
            },
            descriptor_identity: Some("descriptor".into()),
            message: "Complete diagnostic".into(),
        };
        let fields = finding_fields(&finding);
        for expected in [
            ("ViolationKind", "UnsupportedConstraintType"),
            ("ConstraintIdentity", "constraint"),
            ("ConstraintTypeIdentity", "type"),
            ("RuleIdentity", "rule"),
            ("Severity", "Warning"),
            ("SubjectKind", "Relationship"),
            ("HolonIdentity", "source"),
            ("MemberName", "LinkedTo"),
            ("TargetIdentity", "target"),
            ("Message", "Complete diagnostic"),
            ("DescriptorIdentity", "descriptor"),
        ] {
            assert!(fields.iter().any(|(name, value)| *name == expected.0 && value == expected.1));
        }
    }

    #[test]
    fn rule_code_and_transaction_subject_are_projected_without_invented_identities() {
        let finding = CommitValidationViolation {
            kind: CommitValidationViolationKind::RuleViolation { code: "DS-SCHEMA-001".into() },
            rule_key: None,
            severity: ValidationSeverity::Error,
            subject: ValidationSubjectPath::Transaction,
            descriptor_identity: None,
            message: "Cycle".into(),
        };
        let fields = finding_fields(&finding);
        assert!(fields.contains(&("RuleCode", "DS-SCHEMA-001".into())));
        assert!(fields.contains(&("SubjectKind", "Transaction".into())));
        assert!(!fields.iter().any(|(name, _)| *name == "HolonIdentity"));
    }
}
