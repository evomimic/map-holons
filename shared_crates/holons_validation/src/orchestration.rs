use std::{collections::HashSet, ops::Range, sync::Arc};

use core_types::{
    CommitValidationViolation, CommitValidationViolationKind, HolonError, ValidationSubjectPath,
};
use holons_core::core_shared_objects::{holon::ValidationState, transactions::TransactionContext};
use holons_core::{Descriptor, HolonDescriptor, HolonReference, ReadableHolon, StagedReference};

use crate::handlers::finding;
use crate::validators::{resolve_holon_descriptor, validate_described_holon};

use crate::{
    CommitValidationReport, HolonValidationContext, HolonValidationSubject, ValidationCollector,
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
/// Populated authored relationships must be licensed by the completed source
/// contract. These findings join conformance findings in the existing rejection
/// gate, before Commit writes any candidate node or relationship.
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
    let mut assessment = PreparedAssessment::default();
    for candidate in candidates {
        if !candidate.is_live_validation_candidate()? {
            return Err(HolonError::InvalidParameter(format!(
                "Commit validation requires a live staged candidate: {}",
                candidate.reference_id_string()
            )));
        }
        let holon = HolonReference::from(candidate);
        let mut collector = ValidationCollector::default();
        let subject = HolonValidationSubject { holon: &holon };
        if let Some(descriptor) = resolve_holon_descriptor(subject, &mut collector)? {
            validate_authored_relationships(candidate, &descriptor, &mut collector)?;
            validate_described_holon(subject, &descriptor, &validation_context, &mut collector)?;
        }
        // Keep the candidate association directly; finding identities are diagnostics,
        // not lookup keys for installing staged outcomes.
        assessment.record_candidate(candidate, collector.into_report());
    }

    assessment.install_outcomes()
}

/// Keeps installable candidate ranges separate from the complete, flat report.
/// Aggregate findings can be appended to the report without inventing a staged carrier.
#[derive(Default)]
pub(crate) struct PreparedAssessment<'a> {
    report: CommitValidationReport,
    candidate_findings: Vec<(&'a StagedReference, Range<usize>)>,
}

impl<'a> PreparedAssessment<'a> {
    /// Append without shifting existing candidate ranges. Report accumulation must remain
    /// append-only until installation; aggregate findings have no staged outcome association.
    #[allow(dead_code)] // Used by aggregate assessment when C2 activates.
    pub(crate) fn push_aggregate(&mut self, finding: CommitValidationViolation) {
        self.report.unattached_indices.push(self.report.violations.len());
        self.report.violations.push(finding);
    }

    pub(crate) fn record_candidate(
        &mut self,
        candidate: &'a StagedReference,
        report: CommitValidationReport,
    ) {
        let start = self.report.violation_count();
        self.report.violations.extend(report.violations);
        // Keep the reference association: diagnostic identities are not installation keys.
        self.candidate_findings.push((candidate, start..self.report.violation_count()));
    }

    /// Called only after every assessment succeeds; errors before this leave outcomes untouched.
    pub(crate) fn install_outcomes(self) -> Result<CommitValidationReport, HolonError> {
        let Self { report, candidate_findings } = self;
        for (candidate, range) in candidate_findings {
            let findings = report.violations[range].to_vec();
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
            candidate.replace_validation_outcome(state, findings)?;
        }
        Ok(report)
    }
}

/// Checks completed authored state, not a saved holon's navigation surface.
/// The same declared-only contract used by mutation policy licenses these names;
/// no target descriptor or materialized inverse index participates.
fn validate_authored_relationships(
    candidate: &StagedReference,
    descriptor: &HolonDescriptor,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let declared_names = descriptor
        .effective_declared_relationships()?
        .into_iter()
        .map(|declaration| declaration.base_relationship_name().map(|name| name.to_string()))
        .collect::<Result<HashSet<_>, HolonError>>()?;
    let mut relationships = candidate.all_related_holons()?.iter();
    relationships.sort_by_key(|(name, _)| name.to_string());
    for (name, collection) in relationships {
        let members = collection
            .read()
            .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
            .get_members()
            .to_vec();
        if members.is_empty() || declared_names.contains(&name.to_string()) {
            continue;
        }
        for target in members {
            finding(
                collector,
                CommitValidationViolationKind::RuleViolation { code: "UndeclaredRelationship".into() },
                None,
                &ValidationSubjectPath::Relationship {
                    source_identity: candidate.reference_id_string(),
                    name: name.to_string(),
                    target_identity: target.reference_id_string(),
                },
                Some(descriptor.holon().reference_id_string()),
                format!("Populated relationship {name} must be declared by the source's effective descriptor; remove it or author a declared forward relationship."),
            );
        }
    }
    Ok(())
}
