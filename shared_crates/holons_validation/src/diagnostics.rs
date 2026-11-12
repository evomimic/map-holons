use std::collections::BTreeSet;

use core_types::{
    CommitValidationViolation, CommitValidationViolationKind, HolonError, ValidationSeverity,
    ValidationSubjectPath,
};

use crate::commitments::{declaring_identity, required_key};
use crate::ResolvedValidationBinding;

/// Identity-only coverage evidence for report-only corpus conformance assertions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationObservations {
    /// Effective rule keys discovered, including unsupported commitments.
    pub discovered_rule_keys: BTreeSet<String>,
    /// Rule keys whose compatible handlers actually ran.
    pub dispatched_rule_keys: BTreeSet<String>,
    /// Number of effective constraints reached during subject evaluation.
    pub effective_constraint_count: usize,
    /// Scalar evaluator invocations, including incompatible-value-kind declines.
    pub evaluated_value_constraint_count: usize,
    /// Effective attachments inspected by declaration assessment, including reused constraints.
    pub constraint_attachment_count: usize,
    /// Distinct configured constraints declaration-checked in the assessment's Schema scope.
    pub constraint_declaration_count: usize,
    /// Distinct affected Schemas assessed in this Commit attempt.
    pub schema_assessment_count: usize,
}

/// Ordered aggregation owned by the caller, never by a parent holon.
#[derive(Default, Debug)]
pub struct ValidationCollector {
    violations: Vec<CommitValidationViolation>,
    pub(crate) observations: ValidationObservations,
}

impl ValidationCollector {
    /// Appends a finding without sorting or deduplicating distinct contributions.
    pub fn record(&mut self, violation: CommitValidationViolation) {
        self.violations.push(violation);
    }

    /// Coverage evidence remains separate from semantic acceptance.
    pub fn observations(&self) -> &ValidationObservations {
        &self.observations
    }

    pub(crate) fn violations(&self) -> &[CommitValidationViolation] {
        &self.violations
    }

    /// Extracts one completed preparation scope for routing shared definition findings.
    pub(crate) fn take_findings_since(&mut self, start: usize) -> Vec<CommitValidationViolation> {
        self.violations.split_off(start)
    }

    /// Consumes completed findings and observations without copying coverage evidence.
    pub(crate) fn into_parts(self) -> (CommitValidationReport, ValidationObservations) {
        (CommitValidationReport::from_candidate(self.violations), self.observations)
    }

    /// Finishes a successfully completed pass; do not call after an operational error.
    pub fn into_report(self) -> CommitValidationReport {
        CommitValidationReport::from_candidate(self.violations)
    }
}

/// Report and coverage evidence from a completed, report-only Commit assessment.
/// Producing this result leaves staged validation outcomes and content untouched.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitAssessment {
    /// Complete findings, including aggregates without a staged outcome carrier.
    pub report: CommitValidationReport,
    /// Rule discovery, dispatch, and assessment coverage evidence.
    pub observations: crate::ValidationObservations,
}

/// Complete, in-memory assessment containing only identity-based diagnostics.
///
/// This is deliberately not serializable. All currently authored commitments are
/// mandatory, so every finding rejects the assessment regardless of severity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitValidationReport {
    /// All findings, including aggregates without a staged carrier, in traversal
    /// and effective-contribution order. Installation associations belong to orchestration.
    /// Keep this append-only after aggregation so unattached finding positions remain valid.
    pub violations: Vec<CommitValidationViolation>,
    /// Positions of findings without a staged outcome carrier. These remain in the
    /// flat report and are projected separately by the public Commit response.
    pub(crate) unattached_indices: Vec<usize>,
}

impl CommitValidationReport {
    pub(crate) fn from_candidate(violations: Vec<CommitValidationViolation>) -> Self {
        Self { violations, unattached_indices: Vec::new() }
    }

    /// Findings whose subjects have no staged carrier, in report order.
    pub fn unattached_findings(&self) -> Result<Vec<&CommitValidationViolation>, HolonError> {
        self.unattached_indices
            .iter()
            .map(|index| {
                self.violations.get(*index).ok_or_else(|| {
                    HolonError::CommitFailure(format!(
                        "Validation report lost unattached finding at position {index}"
                    ))
                })
            })
            .collect()
    }
    /// Whether the complete assessment has no blocking findings.
    pub fn is_accepted(&self) -> bool {
        self.violations.is_empty()
    }

    /// Number of semantic findings in the complete assessment.
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }
}

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
