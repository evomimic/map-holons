use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

use core_types::{
    CommitValidationViolation, CommitValidationViolationKind, HolonError, ValidationSubjectPath,
};
use holons_core::core_shared_objects::holon::ValidationState;
use holons_core::StagedReference;

use crate::CommitValidationReport;

/// Keeps installable candidate ranges separate from the complete, flat report.
/// Aggregate findings can be appended to the report without inventing a staged carrier.
#[derive(Default)]
pub(crate) struct PreparedAssessment<'a> {
    report: CommitValidationReport,
    candidate_findings: Vec<(&'a StagedReference, Range<usize>)>,
}

impl<'a> PreparedAssessment<'a> {
    /// Partition a multi-subject scope before creating installable ranges. Only exact
    /// live staged subject identities are carriers; all other findings stay unattached.
    pub(crate) fn from_scope(
        candidates: &'a [StagedReference],
        report: CommitValidationReport,
    ) -> Result<Self, HolonError> {
        check_candidates(candidates)?;
        // Diagnostic strings route findings; candidate distinctness uses temporary IDs
        // in the shared input check above.
        let indices: HashMap<_, _> = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| (candidate.reference_id_string(), index))
            .collect();
        let mut groups = vec![Vec::new(); candidates.len()];
        let mut assessment = Self::default();
        for finding in report.violations {
            if let Some(index) =
                subject_identity(&finding.subject).and_then(|identity| indices.get(identity))
            {
                groups[*index].push(finding);
            } else {
                assessment.push_aggregate(finding);
            }
        }
        for (candidate, findings) in candidates.iter().zip(groups) {
            assessment
                .record_candidate(candidate, CommitValidationReport::from_candidate(findings));
        }
        Ok(assessment)
    }

    /// Append without shifting existing candidate ranges. Report accumulation must remain
    /// append-only until installation; aggregate findings have no staged outcome association.
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

    /// Returns the associated report without installing staged outcomes.
    pub(crate) fn into_report(self) -> CommitValidationReport {
        self.report
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

/// The structured primary subject determines the staged carrier, never collector order.
pub(crate) fn subject_identity(subject: &ValidationSubjectPath) -> Option<&str> {
    match subject {
        ValidationSubjectPath::Holon { holon_identity }
        | ValidationSubjectPath::Property { holon_identity, .. }
        | ValidationSubjectPath::Value { holon_identity, .. } => Some(holon_identity),
        ValidationSubjectPath::Relationship { source_identity, .. } => Some(source_identity),
        ValidationSubjectPath::Transaction => None,
    }
}

pub(crate) fn check_candidates(candidates: &[StagedReference]) -> Result<(), HolonError> {
    let mut seen = HashSet::new();
    for candidate in candidates {
        if !candidate.is_live_validation_candidate()? || !seen.insert(candidate.temporary_id()) {
            return Err(HolonError::InvalidParameter(format!(
                "Commit validation requires distinct live candidates: {}",
                candidate.reference_id_string()
            )));
        }
    }
    Ok(())
}
