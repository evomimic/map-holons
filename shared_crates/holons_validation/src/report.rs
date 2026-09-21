use core_types::CommitValidationViolation;

/// Complete, in-memory assessment containing only identity-based diagnostics.
///
/// This is deliberately not serializable. All currently authored commitments are
/// mandatory, so every finding rejects the assessment regardless of severity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitValidationReport {
    /// All findings, including aggregates without a staged carrier, in traversal
    /// and effective-contribution order. Installation associations belong to orchestration.
    pub violations: Vec<CommitValidationViolation>,
}

impl CommitValidationReport {
    /// Whether the complete assessment has no blocking findings.
    pub fn is_accepted(&self) -> bool {
        self.violations.is_empty()
    }

    /// Number of semantic findings in the complete assessment.
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }
}
