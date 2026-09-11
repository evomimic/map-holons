use std::sync::Arc;

use core_types::{CommitValidationViolationKind, HolonError};
use holons_core::core_shared_objects::{holon::ValidationState, transactions::TransactionContext};
use holons_core::{HolonReference, StagedReference};

use crate::{
    validate_holon, CommitValidationReport, HolonValidationContext, HolonValidationSubject,
    ValidationCollector,
};

/// Assesses every supplied live candidate on every call and installs replacement
/// outcomes only after the whole assessment completes.
///
/// The caller derives candidates from the complete Nursery using
/// `StagedReference::is_live_validation_candidate`. All candidates must belong to the
/// supplied transaction and remain unchanged throughout assessment and outcome installation.
/// Previous validation states never exclude a candidate from reassessment.
/// A supplied abandoned or committed entry is a caller error, detected before any
/// outcomes are installed.
///
/// An assessment error leaves all prior outcomes untouched. Completed assessment replaces
/// state and findings together per candidate, preserving operational errors; installation
/// does not provide transaction-wide atomic mutation. An empty set requires no schema anchors.
pub fn validate_commit_candidates(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
) -> Result<CommitValidationReport, HolonError> {
    if candidates.is_empty() {
        return Ok(CommitValidationReport::default());
    }

    let validation_context = HolonValidationContext::resolve(context)?;
    let mut prepared_outcomes = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if !candidate.is_live_validation_candidate()? {
            return Err(HolonError::InvalidParameter(format!(
                "Commit validation requires a live staged candidate: {}",
                candidate.reference_id_string()
            )));
        }
        let holon = HolonReference::from(candidate);
        let mut collector = ValidationCollector::default();
        validate_holon(
            HolonValidationSubject { holon: &holon },
            &validation_context,
            &mut collector,
        )?;
        // Keep the candidate association directly; finding identities are diagnostics,
        // not lookup keys for installing staged outcomes.
        prepared_outcomes.push((candidate, collector.into_report().violations));
    }

    // No staged outcome changes until every candidate has been assessed reliably.
    let mut report = CommitValidationReport::default();
    for (candidate, findings) in prepared_outcomes {
        let state = if findings.is_empty() {
            ValidationState::Validated
        } else if findings
            .iter()
            .any(|finding| matches!(finding.kind, CommitValidationViolationKind::NoDescriptor))
        {
            ValidationState::NoDescriptor
        } else {
            ValidationState::Invalid
        };
        report.violations.extend_from_slice(&findings);
        candidate.replace_validation_outcome(state, findings)?;
    }
    Ok(report)
}
