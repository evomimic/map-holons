use core_types::{CommitValidationViolation, HolonError};

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
