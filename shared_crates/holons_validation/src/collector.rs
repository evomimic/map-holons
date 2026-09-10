use std::collections::BTreeSet;

use core_types::CommitValidationViolation;

use crate::CommitValidationReport;

/// Identity-only coverage evidence for report-only corpus conformance assertions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationObservations {
    /// Effective rule keys discovered, including unsupported commitments.
    pub discovered_rule_keys: BTreeSet<String>,
    /// Rule keys whose compatible handlers actually ran.
    pub dispatched_rule_keys: BTreeSet<String>,
    /// Number of effective constraints reached at C1 subject levels.
    pub effective_constraint_count: usize,
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

    /// Finishes a successfully completed pass; do not call after an operational error.
    pub fn into_report(self) -> CommitValidationReport {
        CommitValidationReport { violations: self.violations }
    }
}
