//! Prospective identity diagnostics for Commit assessment.
use core_types::{
    CommitValidationViolation, CommitValidationViolationKind, HolonError, ValidationSeverity,
    ValidationSubjectPath,
};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::{
    resolve_core_descriptor, AssessmentReadError, HolonReference, ProspectiveDescriptorReader,
};
use std::sync::Arc;

/// Produces one bounded diagnostic per competitor, independent of insertion order.
/// The caller retains candidate associations when installing completed outcomes.
pub fn competing_replacement_findings(
    reader: &ProspectiveDescriptorReader,
) -> Vec<CommitValidationViolation> {
    let mut findings = Vec::new();
    let mut groups: Vec<_> = reader.contested_groups().collect();
    // Compare full identity bytes; diagnostic Display may deliberately abbreviate IDs.
    groups.sort_by(|(left, _), (right, _)| match (left, right) {
        (core_types::HolonId::Local(left), core_types::HolonId::Local(right)) => {
            left.0.cmp(&right.0)
        }
        (core_types::HolonId::Local(_), core_types::HolonId::External(_)) => {
            std::cmp::Ordering::Less
        }
        (core_types::HolonId::External(_), core_types::HolonId::Local(_)) => {
            std::cmp::Ordering::Greater
        }
        (core_types::HolonId::External(left), core_types::HolonId::External(right)) => left
            .space_id
            .0
             .0
            .cmp(&right.space_id.0 .0)
            .then_with(|| left.local_id.0.cmp(&right.local_id.0)),
    });
    for (source, candidates) in groups {
        let mut identities: Vec<_> =
            candidates.iter().map(|candidate| candidate.reference_id_string()).collect();
        identities.sort();
        for (index, identity) in identities.iter().enumerate() {
            let other = &identities[if index == 0 { 1 } else { 0 }];
            findings.push(CommitValidationViolation {
                kind: CommitValidationViolationKind::RuleViolation { code: "CompetingStagedReplacements".into() },
                rule_key: None,
                severity: ValidationSeverity::Error,
                subject: ValidationSubjectPath::Holon { holon_identity: identity.clone() },
                descriptor_identity: None,
                message: format!("Staged candidate {identity} competes for saved source {source} with {} live replacements; another candidate is {other}. Reconcile into one candidate, redirect references, and abandon the others before retrying.", candidates.len()),
            });
        }
    }
    findings
}

/// Resolves a required validation anchor with a deliberate schema-upgrade diagnostic.
/// Resolve anchors through the prospective replacement view.
pub fn resolve_validation_anchor(
    context: &Arc<TransactionContext>,
    key: &str,
) -> Result<HolonReference, AssessmentReadError> {
    classify_anchor_resolution(resolve_core_descriptor(context, key).map_err(Into::into), key)
}

/// Resolves a mandatory anchor without converting contested content to an operational error.
pub fn resolve_validation_anchor_in_view(
    context: &Arc<TransactionContext>,
    key: &str,
    reader: &ProspectiveDescriptorReader,
) -> Result<HolonReference, AssessmentReadError> {
    classify_anchor_resolution(
        holons_core::descriptors::resolve_core_descriptor_with_reader(context, key, reader),
        key,
    )
}

fn classify_anchor_resolution(
    result: Result<HolonReference, AssessmentReadError>,
    key: &str,
) -> Result<HolonReference, AssessmentReadError> {
    match result {
        Err(AssessmentReadError::Operational(HolonError::HolonNotFound(_))) => {
            Err(AssessmentReadError::SchemaIncompatible { missing_anchor: key.into() })
        }
        result => result,
    }
}
