//! Diagnostic projections remain in the same context as their source evidence.
use core_types::{CommitValidationViolation, CommitValidationViolationKind, ValidationSubjectPath};
use holons_prelude::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
static DIAGNOSTIC_SEQUENCE: AtomicU32 = AtomicU32::new(0);

/// Preserve an operational carrier as evidence and expose a common diagnostic shape.
pub(crate) fn operational(
    context: &Arc<TransactionContext>,
    evidence: HolonReference,
) -> Result<HolonReference, HolonError> {
    let mut diagnostic = context.clone_holon(&evidence)?;
    if let Some(descriptor) = crate::response_descriptor::resolve_response_descriptor(
        context,
        "LoadDiagnostic.Projection",
    )? {
        diagnostic.with_descriptor(descriptor)?;
    }
    diagnostic.with_property_value("DiagnosticCategory", MapString("Operational error".into()))?;
    if let Some(message) = evidence.property_value("ErrorMessage")? {
        diagnostic.with_property_value("Message", message)?;
    }
    diagnostic.add_related_holons("DiagnosticEvidence", vec![evidence])?;
    Ok(diagnostic.into())
}

/// Construct an information-preserving finding Node without staging or committing it.
pub(crate) fn finding(
    context: &Arc<TransactionContext>,
    finding: &CommitValidationViolation,
    subject: HolonReference,
) -> Result<HolonReference, HolonError> {
    let mut diagnostic = context.mutation().new_holon(Some(MapString(format!(
        "load-validation-diagnostic-{}",
        DIAGNOSTIC_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))))?;
    if let Some(descriptor) = crate::response_descriptor::resolve_response_descriptor(
        context,
        "LoadDiagnostic.Projection",
    )? {
        diagnostic.with_descriptor(descriptor)?;
    }
    for (name, value) in finding_fields(finding) {
        diagnostic.with_property_value(name, MapString(value))?;
    }
    diagnostic
        .with_property_value("DiagnosticCategory", MapString("Staged validation finding".into()))?;
    diagnostic.add_related_holons("DiagnosticEvidence", vec![subject.clone()])?;
    diagnostic.add_related_holons("DiagnosticSubject", vec![subject])?;
    Ok(diagnostic.into())
}

/// Preserve an unattached Commit finding, including its original carrier relationship.
pub(crate) fn unattached(
    context: &Arc<TransactionContext>,
    evidence: HolonReference,
) -> Result<HolonReference, HolonError> {
    let mut diagnostic = context.clone_holon(&evidence)?;
    if let Some(descriptor) = crate::response_descriptor::resolve_response_descriptor(
        context,
        "LoadDiagnostic.Projection",
    )? {
        diagnostic.with_descriptor(descriptor)?;
    }
    diagnostic.with_property_value(
        "DiagnosticCategory",
        MapString("Unattached validation finding".into()),
    )?;
    diagnostic.add_related_holons("DiagnosticEvidence", vec![evidence])?;
    Ok(diagnostic.into())
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

    fn context_with_descriptor() -> Result<Arc<TransactionContext>, HolonError> {
        let context = crate::controller::tests::context();
        let descriptor = context.mutation().new_holon(Some("LoadDiagnostic.Projection".into()))?;
        context.mutation().stage_new_holon(descriptor)?;
        Ok(context)
    }

    #[test]
    fn diagnostic_preserves_fields_and_original_staged_subject_in_its_own_context(
    ) -> Result<(), HolonError> {
        let context = context_with_descriptor()?;
        let source = context.mutation().new_holon(Some("affected".into()))?;
        let subject: HolonReference = context.mutation().stage_new_holon(source)?.into();
        let evidence = CommitValidationViolation {
            kind: CommitValidationViolationKind::UnsupportedConstraintType {
                constraint_identity: "constraint".into(),
                constraint_type_identity: "constraint-type".into(),
            },
            rule_key: Some("rule".into()),
            severity: core_types::ValidationSeverity::Warning,
            subject: ValidationSubjectPath::Relationship {
                source_identity: "source".into(),
                name: "Members".into(),
                target_identity: "target".into(),
            },
            descriptor_identity: Some("descriptor".into()),
            message: "full evidence".into(),
        };
        let diagnostic = finding(&context, &evidence, subject.clone())?;
        assert!(matches!(diagnostic, HolonReference::Transient(_)));
        for (name, value) in [
            ("ConstraintIdentity", "constraint"),
            ("ConstraintTypeIdentity", "constraint-type"),
            ("RuleIdentity", "rule"),
            ("Severity", "Warning"),
            ("SubjectKind", "Relationship"),
            ("HolonIdentity", "source"),
            ("MemberName", "Members"),
            ("TargetIdentity", "target"),
            ("DescriptorIdentity", "descriptor"),
            ("Message", "full evidence"),
        ] {
            assert_eq!(
                diagnostic.property_value(name)?,
                Some(BaseValue::StringValue(value.into()))
            );
        }
        for name in ["DiagnosticSubject", "DiagnosticEvidence"] {
            let targets = diagnostic.related_holons(name)?;
            let targets = targets.read().unwrap();
            assert_eq!(targets.get_members().len(), 1);
            assert_eq!(targets.get_members()[0], subject);
            if let HolonReference::Staged(target) = &targets.get_members()[0] {
                assert_eq!(target.tx_id(), context.tx_id());
            } else {
                panic!("affected subject lost its staged phase");
            }
        }
        assert_ne!(diagnostic, finding(&context, &evidence, subject)?);
        Ok(())
    }

    #[test]
    fn operational_diagnostic_retains_original_carrier_and_all_authored_evidence(
    ) -> Result<(), HolonError> {
        let context = context_with_descriptor()?;
        let mut carrier = context.mutation().new_holon(Some("error".into()))?;
        carrier.with_property_value("ErrorMessage", "original message")?;
        carrier.with_property_value("ErrorType", "specific error")?;
        carrier.with_property_value("Filename", "/source.json")?;
        carrier.with_property_value("StartUtf8ByteOffset", 0_i64)?;
        let evidence: HolonReference = carrier.into();
        let diagnostic = operational(&context, evidence.clone())?;
        assert_eq!(diagnostic.property_value("Message")?, evidence.property_value("ErrorMessage")?);
        assert_eq!(
            diagnostic.property_value("StartUtf8ByteOffset")?,
            evidence.property_value("StartUtf8ByteOffset")?
        );
        assert_eq!(
            diagnostic.related_holons("DiagnosticEvidence")?.read().unwrap().get_members(),
            &vec![evidence]
        );
        assert!(diagnostic
            .related_holons("DiagnosticSubject")?
            .read()
            .unwrap()
            .get_members()
            .is_empty());
        Ok(())
    }
}
